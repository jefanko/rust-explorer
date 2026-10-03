use crate::state::AppState;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken, SessionId};
use explorer_domain::models::{
    BootstrapData, DirectoryPage, NavigationResponse, SortColumn, SortDirection,
};
use explorer_win::known_folders::{get_logical_drives, get_standard_known_folders};
use explorer_win::shell::open_file_with_association;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> Result<BootstrapData, ExplorerError> {
    let known_folders = tokio::task::spawn_blocking(get_standard_known_folders)
        .await
        .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", "bootstrap"))?;

    let drives = tokio::task::spawn_blocking(get_logical_drives)
        .await
        .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", "bootstrap"))?;

    let initial_path = known_folders
        .iter()
        .find(|f| f.id == "documents")
        .map(|f| f.path.clone())
        .or_else(|| drives.first().map(|d| d.path.clone()))
        .unwrap_or_else(|| r"C:\".to_string());

    let _ = state.folder_service.navigate(Path::new(&initial_path));

    Ok(BootstrapData {
        session_id: SessionId::new(),
        known_folders,
        drives,
        initial_path,
    })
}

#[tauri::command]
pub async fn navigate(
    path: String,
    state: State<'_, AppState>,
) -> Result<NavigationResponse, ExplorerError> {
    let folder_svc = state.folder_service.clone();
    tokio::task::spawn_blocking(move || folder_svc.navigate(Path::new(&path)))
        .await
        .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", "navigate"))?
}

#[tauri::command]
pub async fn list_page(
    folder_token: FolderToken,
    generation: u64,
    offset: usize,
    limit: usize,
    sort_column: Option<SortColumn>,
    sort_direction: Option<SortDirection>,
    state: State<'_, AppState>,
) -> Result<DirectoryPage, ExplorerError> {
    let col = sort_column.unwrap_or_default();
    let dir = sort_direction.unwrap_or_default();
    let folder_svc = state.folder_service.clone();

    tokio::task::spawn_blocking(move || {
        folder_svc.list_page(&folder_token, generation, offset, limit, col, dir)
    })
    .await
    .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", "list_page"))?
}

#[tauri::command]
pub async fn refresh(
    folder_token: FolderToken,
    state: State<'_, AppState>,
) -> Result<NavigationResponse, ExplorerError> {
    let folder_svc = state.folder_service.clone();

    tokio::task::spawn_blocking(move || {
        let path = folder_svc.get_folder_path(&folder_token).ok_or_else(|| {
            ExplorerError::new(ErrorCode::StaleItem, "Folder token not found", "refresh")
        })?;
        folder_svc.navigate(&path)
    })
    .await
    .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", "refresh"))?
}

#[tauri::command]
pub async fn open_item(
    folder_token: FolderToken,
    item_token: ItemToken,
    state: State<'_, AppState>,
) -> Result<Option<NavigationResponse>, ExplorerError> {
    let folder_svc = state.folder_service.clone();

    let item_path = folder_svc
        .resolve_item(&folder_token, &item_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                "Item token not found in snapshot",
                "open_item",
            )
        })?;

    if item_path.is_dir() {
        // If it's a directory, navigate into it and return the new NavigationResponse
        let nav = tokio::task::spawn_blocking(move || folder_svc.navigate(&item_path))
            .await
            .map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Worker join failed", "open_item")
            })??;
        Ok(Some(nav))
    } else {
        // If it's a file, launch with Windows association
        tokio::task::spawn_blocking(move || open_file_with_association(&item_path))
            .await
            .map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Worker join failed", "open_item")
            })??;
        Ok(None)
    }
}
