use crate::state::AppState;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken, SessionId};
use explorer_domain::models::{
    BootstrapData, DirectoryPage, NavigationResponse, PreviewData, SortColumn, SortDirection,
};
use explorer_domain::operations::{JobSummary, OperationPlan};
use explorer_store::AppSettings;
use explorer_win::known_folders::{get_logical_drives, get_standard_known_folders};
use explorer_win::shell::{
    open_file_with_association, open_in_windows_explorer, show_file_properties,
};
use std::path::{Path, PathBuf};
use tauri::State;

async fn run_blocking<T, F>(operation: &'static str, work: F) -> Result<T, ExplorerError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ExplorerError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", operation))?
}

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
pub async fn navigate_native_path(
    path_utf16: Vec<u16>,
    state: State<'_, AppState>,
) -> Result<NavigationResponse, ExplorerError> {
    let path = path_from_utf16(path_utf16, "navigate_native_path")?;
    let folder_svc = state.folder_service.clone();
    tokio::task::spawn_blocking(move || folder_svc.navigate(&path))
        .await
        .map_err(|_| {
            ExplorerError::new(
                ErrorCode::Internal,
                "Worker join failed",
                "navigate_native_path",
            )
        })?
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

#[tauri::command]
pub async fn plan_create_folder(
    folder_token: FolderToken,
    name: String,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    let parent = state
        .folder_service
        .get_folder_path(&folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                "Folder token not found",
                "plan_create_folder",
            )
        })?;

    let operation_service = state.operation_service.clone();
    run_blocking("plan_create_folder", move || {
        operation_service.plan_create_folder(&parent, &name)
    })
    .await
}

#[tauri::command]
pub async fn plan_rename(
    folder_token: FolderToken,
    item_token: ItemToken,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    let source = state
        .folder_service
        .resolve_item(&folder_token, &item_token)
        .ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item token not found", "plan_rename")
        })?;

    let operation_service = state.operation_service.clone();
    run_blocking("plan_rename", move || {
        operation_service.plan_rename(&source, &new_name)
    })
    .await
}

#[tauri::command]
pub async fn commit_plan(
    plan_id: explorer_domain::ids::PlanId,
    state: State<'_, AppState>,
) -> Result<JobSummary, ExplorerError> {
    let operation_service = state.operation_service.clone();
    run_blocking("commit_plan", move || {
        operation_service.commit_plan(&plan_id)
    })
    .await
}

#[tauri::command]
pub async fn create_folder(
    folder_token: FolderToken,
    name: String,
    state: State<'_, AppState>,
) -> Result<JobSummary, ExplorerError> {
    let parent = state
        .folder_service
        .get_folder_path(&folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                "Folder token not found",
                "create_folder",
            )
        })?;

    let operation_service = state.operation_service.clone();
    run_blocking("create_folder", move || {
        operation_service.execute_create_folder(&parent, &name)
    })
    .await
}

#[tauri::command]
pub async fn rename_item(
    folder_token: FolderToken,
    item_token: ItemToken,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<JobSummary, ExplorerError> {
    let source = state
        .folder_service
        .resolve_item(&folder_token, &item_token)
        .ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item token not found", "rename_item")
        })?;

    let operation_service = state.operation_service.clone();
    run_blocking("rename_item", move || {
        operation_service.execute_rename(&source, &new_name)
    })
    .await
}

#[tauri::command]
pub async fn list_jobs(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<JobSummary>, ExplorerError> {
    state
        .operation_service
        .list_recent_jobs(limit.unwrap_or(20))
}

#[tauri::command]
pub async fn plan_paste(
    destination_folder_token: FolderToken,
    state: State<'_, AppState>,
) -> Result<Option<OperationPlan>, ExplorerError> {
    let folder_service = state.folder_service.clone();
    let operation_service = state.operation_service.clone();
    run_blocking("plan_paste", move || {
        let Some((source_paths, is_cut)) = explorer_win::clipboard::read_clipboard_hdrop()? else {
            return Ok(None);
        };
        let destination = folder_service
            .get_folder_path(&destination_folder_token)
            .ok_or_else(|| {
                ExplorerError::new(
                    ErrorCode::StaleItem,
                    "Destination folder token expired",
                    "plan_paste",
                )
            })?;
        let plan = if is_cut {
            operation_service.plan_move(&source_paths, &destination)?
        } else {
            operation_service.plan_copy(&source_paths, &destination)?
        };
        Ok(Some(plan))
    })
    .await
}

#[tauri::command]
pub async fn plan_recycle(
    folder_token: FolderToken,
    item_tokens: Vec<ItemToken>,
    state: State<'_, AppState>,
) -> Result<OperationPlan, ExplorerError> {
    let sources = item_tokens
        .iter()
        .map(|token| {
            state
                .folder_service
                .resolve_item(&folder_token, token)
                .ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::StaleItem,
                        "Selected item token expired",
                        "plan_recycle",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let operation_service = state.operation_service.clone();
    run_blocking("plan_recycle", move || {
        operation_service.plan_recycle(&sources)
    })
    .await
}

/// Resolves the tokens of a drag & drop request into native paths.
/// The destination is a folder row (`destination_item_token`) inside the destination folder,
/// or the destination folder itself when no item token is supplied.
fn resolve_transfer_paths(
    folder_service: &explorer_fs::FolderService,
    source_folder_token: &FolderToken,
    item_tokens: &[ItemToken],
    destination_folder_token: &FolderToken,
    destination_item_token: Option<&ItemToken>,
) -> Result<(Vec<PathBuf>, PathBuf), ExplorerError> {
    const OP: &str = "plan_transfer";
    if item_tokens.is_empty() {
        return Err(ExplorerError::new(
            ErrorCode::UnsupportedPath,
            "No items selected to drop",
            OP,
        ));
    }
    let sources = item_tokens
        .iter()
        .map(|token| {
            folder_service
                .resolve_item(source_folder_token, token)
                .ok_or_else(|| {
                    ExplorerError::new(ErrorCode::StaleItem, "Selected item token expired", OP)
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let destination = match destination_item_token {
        Some(token) => {
            let path = folder_service
                .resolve_item(destination_folder_token, token)
                .ok_or_else(|| {
                    ExplorerError::new(ErrorCode::StaleItem, "Drop target token expired", OP)
                })?;
            if !path.is_dir() {
                return Err(ExplorerError::new(
                    ErrorCode::UnsupportedPath,
                    "Drop target is not a folder",
                    OP,
                ));
            }
            path
        }
        None => folder_service
            .get_folder_path(destination_folder_token)
            .ok_or_else(|| {
                ExplorerError::new(ErrorCode::StaleItem, "Destination folder token expired", OP)
            })?,
    };
    Ok((sources, destination))
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
    let folder_service = state.folder_service.clone();
    let operation_service = state.operation_service.clone();
    run_blocking("plan_transfer", move || {
        let (sources, destination) = resolve_transfer_paths(
            &folder_service,
            &source_folder_token,
            &item_tokens,
            &destination_folder_token,
            destination_item_token.as_ref(),
        )?;
        explorer_jobs::planner::validate_drop_request(&sources, &destination, is_move)?;
        if is_move {
            operation_service.plan_move(&sources, &destination)
        } else {
            operation_service.plan_copy(&sources, &destination)
        }
    })
    .await
}

#[tauri::command]
pub async fn read_preview(
    folder_token: FolderToken,
    item_token: ItemToken,
    state: State<'_, AppState>,
) -> Result<PreviewData, ExplorerError> {
    let path = state
        .folder_service
        .resolve_item(&folder_token, &item_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::StaleItem,
                "Selected item token expired",
                "read_preview",
            )
        })?;
    run_blocking("read_preview", move || {
        Ok(explorer_fs::preview::build_preview(&path))
    })
    .await
}

#[tauri::command]
pub async fn clipboard_write_items(
    window: tauri::Window,
    folder_token: FolderToken,
    item_tokens: Vec<ItemToken>,
    is_cut: bool,
    state: State<'_, AppState>,
) -> Result<(), ExplorerError> {
    let paths = item_tokens
        .iter()
        .map(|token| {
            state
                .folder_service
                .resolve_item(&folder_token, token)
                .ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::StaleItem,
                        "Selected item token expired",
                        "clipboard_write_items",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let owner = clipboard_owner(&window)?;
    run_blocking("clipboard_write_items", move || {
        explorer_win::clipboard::write_clipboard_hdrop(&paths, is_cut, owner)
    })
    .await
}

#[tauri::command]
pub async fn clipboard_write_path(
    window: tauri::Window,
    path_utf16: Vec<u16>,
) -> Result<(), ExplorerError> {
    let owner = clipboard_owner(&window)?;
    run_blocking("clipboard_write_path", move || {
        explorer_win::clipboard::write_clipboard_text_utf16(&path_utf16, owner)
    })
    .await
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
    let p = state
        .folder_service
        .get_folder_path(&folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::StaleItem,
                "Folder token expired before watch registration",
                "watch_folder",
            )
        })?;
    state
        .watch_service
        .subscribe(&p, explorer_watch::WatchMode::NonRecursive, &subscriber_id)
}

#[tauri::command]
pub async fn unwatch_all(
    state: State<'_, AppState>,
    subscriber_id: String,
) -> Result<(), ExplorerError> {
    state.watch_service.unsubscribe_all(&subscriber_id)
}

#[tauri::command]
pub async fn list_indexed_roots(
    state: State<'_, AppState>,
) -> Result<Vec<explorer_index::IndexedRoot>, ExplorerError> {
    state.index_service.list_roots()
}

#[tauri::command]
pub async fn add_indexed_root(
    state: State<'_, AppState>,
    folder_token: FolderToken,
) -> Result<explorer_index::IndexedRoot, ExplorerError> {
    let p = state
        .folder_service
        .get_folder_path(&folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::StaleItem,
                "Folder token expired before indexing",
                "add_indexed_root",
            )
        })?;
    let index_service = state.index_service.clone();
    let watch_service = state.watch_service.clone();
    run_blocking("add_indexed_root", move || {
        let root = index_service.register_root(&p)?;
        let subscriber_id = format!("indexed-root:{}", root.id);
        if let Err(error) = watch_service.subscribe(
            &root.path,
            explorer_watch::WatchMode::Recursive,
            &subscriber_id,
        ) {
            let _ = index_service.remove_root(&root.id);
            return Err(error);
        }
        if let Err(error) = index_service.recrawl_root(&root.id) {
            let _ = watch_service.unsubscribe_all(&subscriber_id);
            let _ = index_service.remove_root(&root.id);
            return Err(error);
        }
        Ok(root)
    })
    .await
}

#[tauri::command]
pub async fn remove_indexed_root(
    state: State<'_, AppState>,
    root_id: String,
) -> Result<(), ExplorerError> {
    let index_service = state.index_service.clone();
    let watch_service = state.watch_service.clone();
    run_blocking("remove_indexed_root", move || {
        let subscriber_id = format!("indexed-root:{root_id}");
        watch_service.unsubscribe_all(&subscriber_id)?;
        index_service.remove_root(&root_id)
    })
    .await
}

#[tauri::command]
pub async fn recrawl_indexed_root(
    state: State<'_, AppState>,
    root_id: String,
) -> Result<(), ExplorerError> {
    let index_service = state.index_service.clone();
    run_blocking("recrawl_indexed_root", move || {
        index_service.recrawl_root(&root_id)
    })
    .await
}

#[tauri::command]
pub async fn search_indexed(
    state: State<'_, AppState>,
    query: String,
    root_id: Option<String>,
    page: usize,
    page_size: usize,
) -> Result<explorer_index::SearchResponse, ExplorerError> {
    state
        .index_service
        .search(&query, root_id.as_deref(), page, page_size)
}

#[tauri::command]
pub async fn open_path(
    path_utf16: Vec<u16>,
    state: State<'_, AppState>,
) -> Result<Option<NavigationResponse>, ExplorerError> {
    let p = path_from_utf16(path_utf16, "open_path")?;
    if !p.exists() {
        return Err(ExplorerError::new(
            ErrorCode::NotFound,
            format!("Path does not exist: {}", p.display()),
            "open_path",
        ));
    }
    let folder_svc = state.folder_service.clone();
    if p.is_dir() {
        let nav = tokio::task::spawn_blocking(move || folder_svc.navigate(&p))
            .await
            .map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Worker join failed", "open_path")
            })??;
        Ok(Some(nav))
    } else {
        tokio::task::spawn_blocking(move || open_file_with_association(&p))
            .await
            .map_err(|_| {
                ExplorerError::new(ErrorCode::Internal, "Worker join failed", "open_path")
            })??;
        Ok(None)
    }
}

fn path_from_utf16(
    path_utf16: Vec<u16>,
    operation: &'static str,
) -> Result<PathBuf, ExplorerError> {
    use std::os::windows::ffi::OsStringExt;
    if path_utf16.is_empty() || path_utf16.contains(&0) {
        return Err(ExplorerError::new(
            ErrorCode::InvalidName,
            "Native path is empty or contains an embedded NUL",
            operation,
        ));
    }
    let p = PathBuf::from(std::ffi::OsString::from_wide(&path_utf16));
    Ok(explorer_win::path::normalize_drive_root(&p))
}
