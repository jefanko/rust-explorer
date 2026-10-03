use crate::state::AppState;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken, SessionId};
use explorer_domain::models::{
    BootstrapData, DirectoryPage, NavigationResponse, SortColumn, SortDirection,
};
use explorer_store::AppSettings;
use explorer_win::known_folders::{get_logical_drives, get_standard_known_folders};
use explorer_win::shell::{
    open_file_with_association, open_in_windows_explorer, show_file_properties,
};
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

    // Load saved settings
    let settings = state.settings_store.load_settings().unwrap_or_default();

    let initial_path = if settings.restore_tabs && !settings.saved_tabs.is_empty() {
        settings.saved_tabs[0].clone()
    } else {
        known_folders
            .iter()
            .find(|f| f.id == "documents")
            .map(|f| f.path.clone())
            .or_else(|| drives.first().map(|d| d.path.clone()))
            .unwrap_or_else(|| r"C:\".to_string())
    };

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
        let nav = tokio::task::spawn_blocking(move || folder_svc.navigate(&item_path))
            .await
            .map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Worker join failed", "open_item")
            })??;
        Ok(Some(nav))
    } else {
        tokio::task::spawn_blocking(move || open_file_with_association(&item_path))
            .await
            .map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Worker join failed", "open_item")
            })??;
        Ok(None)
    }
}

#[tauri::command]
pub async fn show_properties(
    folder_token: FolderToken,
    item_token: Option<ItemToken>,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    let folder_svc = state.folder_service.clone();

    let path = if let Some(it) = item_token {
        folder_svc.resolve_item(&folder_token, &it).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item not found", "show_properties")
        })?
    } else {
        folder_svc.get_folder_path(&folder_token).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Folder not found", "show_properties")
        })?
    };

    tokio::task::spawn_blocking(move || show_file_properties(&path))
        .await
        .map_err(|_| {
            ExplorerError::new(ErrorCode::Internal, "Worker join failed", "show_properties")
        })?
}

#[tauri::command]
pub async fn open_in_explorer(
    folder_token: FolderToken,
    item_token: Option<ItemToken>,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    let folder_svc = state.folder_service.clone();

    let path = if let Some(it) = item_token {
        folder_svc.resolve_item(&folder_token, &it).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item not found", "open_in_explorer")
        })?
    } else {
        folder_svc.get_folder_path(&folder_token).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Folder not found", "open_in_explorer")
        })?
    };

    tokio::task::spawn_blocking(move || open_in_windows_explorer(&path))
        .await
        .map_err(|_| {
            ExplorerError::new(
                ErrorCode::Internal,
                "Worker join failed",
                "open_in_explorer",
            )
        })?
}

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<AppSettings, ExplorerError> {
    state.settings_store.load_settings()
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    state.settings_store.save_settings(&settings)
}

#[tauri::command]
pub async fn add_favorite(path: String, state: State<'_, AppState>) -> Result<(), ExplorerError> {
    state.settings_store.add_favorite(&path)
}

#[tauri::command]
pub async fn remove_favorite(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    state.settings_store.remove_favorite(&path)
}
