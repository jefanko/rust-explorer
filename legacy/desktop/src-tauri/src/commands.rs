use crate::state::AppState;
use explorer_app::operations;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken, PlanId};
use explorer_domain::models::{
    BootstrapData, DirectoryPage, NavigationResponse, PreviewData, SortColumn, SortDirection,
};
use explorer_domain::operations::{JobSummary, OperationPlan};
use explorer_store::AppSettings;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> Result<BootstrapData, ExplorerError> {
    operations::bootstrap(&state).await
}

#[tauri::command]
pub async fn navigate(
    path: String,
    state: State<'_, AppState>,
) -> Result<NavigationResponse, ExplorerError> {
    operations::navigate(&state, Path::new(&path)).await
}

#[tauri::command]
pub async fn navigate_native_path(
    path_utf16: Vec<u16>,
    state: State<'_, AppState>,
) -> Result<NavigationResponse, ExplorerError> {
    operations::navigate_native_path(&state, &path_utf16).await
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
    operations::list_page(
        &state,
        &folder_token,
        generation,
        offset,
        limit,
        sort_column,
        sort_direction,
    )
    .await
}

#[tauri::command]
pub async fn refresh(
    folder_token: FolderToken,
    state: State<'_, AppState>,
) -> Result<NavigationResponse, ExplorerError> {
    operations::refresh(&state, &folder_token).await
}

#[tauri::command]
pub async fn open_item(
    folder_token: FolderToken,
    item_token: ItemToken,
    state: State<'_, AppState>,
) -> Result<Option<NavigationResponse>, ExplorerError> {
    operations::open_item(&state, &folder_token, &item_token).await
}

#[tauri::command]
pub async fn show_properties(
    folder_token: FolderToken,
    item_token: Option<ItemToken>,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    operations::show_properties(&state, &folder_token, item_token.as_ref()).await
}

#[tauri::command]
pub async fn open_in_explorer(
    folder_token: FolderToken,
    item_token: Option<ItemToken>,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    operations::open_in_explorer(&state, &folder_token, item_token.as_ref()).await
}

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<AppSettings, ExplorerError> {
    operations::load_settings(&state)
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    operations::save_settings(&state, &settings)
}

#[tauri::command]
pub async fn add_favorite(path: String, state: State<'_, AppState>) -> Result<(), ExplorerError> {
    operations::add_favorite(&state, &path)
}

#[tauri::command]
pub async fn remove_favorite(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    operations::remove_favorite(&state, &path)
}

#[tauri::command]
pub async fn plan_create_folder(
    folder_token: FolderToken,
    name: String,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    operations::plan_create_folder(&state, &folder_token, &name).await
}

#[tauri::command]
pub async fn plan_rename(
    folder_token: FolderToken,
    item_token: ItemToken,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    operations::plan_rename(&state, &folder_token, &item_token, &new_name).await
}

#[tauri::command]
pub async fn commit_plan(
    plan_id: PlanId,
    state: State<'_, AppState>,
) -> Result<JobSummary, ExplorerError> {
    operations::commit_plan(&state, &plan_id).await
}

#[tauri::command]
pub async fn create_folder(
    folder_token: FolderToken,
    name: String,
    state: State<'_, AppState>,
) -> Result<JobSummary, ExplorerError> {
    operations::create_folder(&state, &folder_token, &name).await
}

#[tauri::command]
pub async fn rename_item(
    folder_token: FolderToken,
    item_token: ItemToken,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<JobSummary, ExplorerError> {
    operations::rename_item(&state, &folder_token, &item_token, &new_name).await
}

#[tauri::command]
pub async fn list_jobs(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<JobSummary>, ExplorerError> {
    operations::list_jobs(&state, limit)
}

#[tauri::command]
pub async fn plan_paste(
    destination_folder_token: FolderToken,
    state: State<'_, AppState>,
) -> Result<Option<OperationPlan>, ExplorerError> {
    operations::plan_paste(&state, &destination_folder_token).await
}

#[tauri::command]
pub async fn plan_recycle(
    folder_token: FolderToken,
    item_tokens: Vec<ItemToken>,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    operations::plan_recycle(&state, &folder_token, &item_tokens).await
}

#[tauri::command]
pub async fn plan_transfer(
    source_folder_token: FolderToken,
    item_tokens: Vec<ItemToken>,
    destination_folder_token: FolderToken,
    destination_item_token: Option<ItemToken>,
    is_move: bool,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    operations::plan_transfer(
        &state,
        &source_folder_token,
        &item_tokens,
        &destination_folder_token,
        destination_item_token.as_ref(),
        is_move,
    )
    .await
}

#[tauri::command]
pub async fn read_preview(
    folder_token: FolderToken,
    item_token: ItemToken,
    state: State<'_, AppState>,
) -> Result<PreviewData, ExplorerError> {
    operations::read_preview(&state, &folder_token, &item_token).await
}

#[tauri::command]
pub async fn clipboard_write_items(
    window: tauri::Window,
    folder_token: FolderToken,
    item_tokens: Vec<ItemToken>,
    is_cut: bool,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    let owner = clipboard_owner(&window)?;
    operations::clipboard_write_items(&state, &folder_token, &item_tokens, is_cut, owner).await
}

#[tauri::command]
pub async fn clipboard_write_path(
    window: tauri::Window,
    path_utf16: Vec<u16>,
) -> Result<(), ExplorerError> {
    let owner = clipboard_owner(&window)?;
    operations::clipboard_write_path(&path_utf16, owner).await
}

fn clipboard_owner(window: &tauri::Window) -> Result<usize, ExplorerError> {
    if window.label() != "main" {
        return Err(ExplorerError::new(
            ErrorCode::AccessDenied,
            "Clipboard owner must be the authorized main window",
            "clipboard_owner",
        ));
    }
    window.hwnd().map(|hwnd| hwnd.0 as usize).map_err(|e| {
        ExplorerError::new(
            ErrorCode::Internal,
            format!("Clipboard window is unavailable: {e}"),
            "clipboard_owner",
        )
    })
}

#[tauri::command]
pub async fn watch_folder(
    state: State<'_, AppState>,
    folder_token: FolderToken,
    subscriber_id: String,
) -> Result<(), ExplorerError> {
    operations::watch_folder(&state, &folder_token, &subscriber_id)
}

#[tauri::command]
pub async fn unwatch_all(
    state: State<'_, AppState>,
    subscriber_id: String,
) -> Result<(), ExplorerError> {
    operations::unwatch_all(&state, &subscriber_id)
}

#[tauri::command]
pub async fn list_indexed_roots(
    state: State<'_, AppState>,
) -> Result<Vec<explorer_index::IndexedRoot>, ExplorerError> {
    operations::list_indexed_roots(&state)
}

#[tauri::command]
pub async fn add_indexed_root(
    state: State<'_, AppState>,
    folder_token: FolderToken,
) -> Result<explorer_index::IndexedRoot, ExplorerError> {
    operations::add_indexed_root(&state, &folder_token).await
}

#[tauri::command]
pub async fn remove_indexed_root(
    state: State<'_, AppState>,
    root_id: String,
) -> Result<(), ExplorerError> {
    operations::remove_indexed_root(&state, &root_id).await
}

#[tauri::command]
pub async fn recrawl_indexed_root(
    state: State<'_, AppState>,
    root_id: String,
) -> Result<(), ExplorerError> {
    operations::recrawl_indexed_root(&state, &root_id).await
}

#[tauri::command]
pub async fn search_indexed(
    state: State<'_, AppState>,
    query: String,
    root_id: Option<String>,
    page: usize,
    page_size: usize,
) -> Result<explorer_index::SearchResponse, ExplorerError> {
    operations::search_indexed(&state, &query, root_id.as_deref(), page, page_size)
}

#[tauri::command]
pub async fn open_path(
    path_utf16: Vec<u16>,
    state: State<'_, AppState>,
) -> Result<Option<NavigationResponse>, ExplorerError> {
    operations::open_path(&state, &path_utf16).await
}
