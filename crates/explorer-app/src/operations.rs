use crate::state::AppState;
use explorer_domain::errors::{ErrorCode, ExplorerError};
use explorer_domain::ids::{FolderToken, ItemToken, PlanId, SessionId};
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

pub async fn run_blocking<T, F>(operation: &'static str, work: F) -> Result<T, ExplorerError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ExplorerError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ExplorerError::new(ErrorCode::Internal, "Worker join failed", operation))?
}

pub async fn bootstrap(state: &AppState) -> Result<BootstrapData, ExplorerError> {
    let known_folders = run_blocking("bootstrap", || Ok(get_standard_known_folders())).await?;
    let drives = run_blocking("bootstrap", || Ok(get_logical_drives())).await?;

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

pub async fn navigate(state: &AppState, path: &Path) -> Result<NavigationResponse, ExplorerError> {
    navigate_with(state, path, true).await
}

/// Navigates with an explicit hidden/system-entry policy (see `FolderService::navigate_with`).
pub async fn navigate_with(
    state: &AppState,
    path: &Path,
    include_hidden: bool,
) -> Result<NavigationResponse, ExplorerError> {
    let folder_svc = state.folder_service.clone();
    let p = path.to_path_buf();
    run_blocking("navigate", move || {
        folder_svc.navigate_with(&p, include_hidden)
    })
    .await
}

pub async fn navigate_native_path(
    state: &AppState,
    path_utf16: &[u16],
) -> Result<NavigationResponse, ExplorerError> {
    navigate_native_path_with(state, path_utf16, true).await
}

pub async fn navigate_native_path_with(
    state: &AppState,
    path_utf16: &[u16],
    include_hidden: bool,
) -> Result<NavigationResponse, ExplorerError> {
    let path = path_from_utf16(path_utf16, "navigate_native_path")?;
    let folder_svc = state.folder_service.clone();
    run_blocking("navigate_native_path", move || {
        folder_svc.navigate_with(&path, include_hidden)
    })
    .await
}

pub async fn list_page(
    state: &AppState,
    folder_token: &FolderToken,
    generation: u64,
    offset: usize,
    limit: usize,
    sort_column: Option<SortColumn>,
    sort_direction: Option<SortDirection>,
) -> Result<DirectoryPage, ExplorerError> {
    let col = sort_column.unwrap_or_default();
    let dir = sort_direction.unwrap_or_default();
    let folder_svc = state.folder_service.clone();
    let ft = folder_token.clone();

    run_blocking("list_page", move || {
        folder_svc.list_page(&ft, generation, offset, limit, col, dir)
    })
    .await
}

pub async fn refresh(
    state: &AppState,
    folder_token: &FolderToken,
) -> Result<NavigationResponse, ExplorerError> {
    refresh_with(state, folder_token, true).await
}

pub async fn refresh_with(
    state: &AppState,
    folder_token: &FolderToken,
    include_hidden: bool,
) -> Result<NavigationResponse, ExplorerError> {
    let folder_svc = state.folder_service.clone();
    let ft = folder_token.clone();

    run_blocking("refresh", move || {
        let path = folder_svc.get_folder_path(&ft).ok_or_else(|| {
            ExplorerError::new(ErrorCode::StaleItem, "Folder token not found", "refresh")
        })?;
        folder_svc.navigate_with(&path, include_hidden)
    })
    .await
}

pub async fn open_item(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: &ItemToken,
) -> Result<Option<NavigationResponse>, ExplorerError> {
    open_item_with(state, folder_token, item_token, true).await
}

pub async fn open_item_with(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: &ItemToken,
    include_hidden: bool,
) -> Result<Option<NavigationResponse>, ExplorerError> {
    let folder_svc = state.folder_service.clone();

    let item_path = folder_svc
        .resolve_item(folder_token, item_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                "Item token not found in snapshot",
                "open_item",
            )
        })?;

    if item_path.is_dir() {
        let nav = run_blocking("open_item", move || {
            folder_svc.navigate_with(&item_path, include_hidden)
        })
        .await?;
        Ok(Some(nav))
    } else {
        run_blocking("open_item", move || open_file_with_association(&item_path)).await?;
        Ok(None)
    }
}

pub async fn show_properties(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: Option<&ItemToken>,
) -> Result<(), ExplorerError> {
    let folder_svc = state.folder_service.clone();

    let path = if let Some(it) = item_token {
        folder_svc.resolve_item(folder_token, it).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item not found", "show_properties")
        })?
    } else {
        folder_svc.get_folder_path(folder_token).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Folder not found", "show_properties")
        })?
    };

    run_blocking("show_properties", move || show_file_properties(&path)).await
}

pub async fn open_in_explorer(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: Option<&ItemToken>,
) -> Result<(), ExplorerError> {
    let folder_svc = state.folder_service.clone();

    let path = if let Some(it) = item_token {
        folder_svc.resolve_item(folder_token, it).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item not found", "open_in_explorer")
        })?
    } else {
        folder_svc.get_folder_path(folder_token).ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Folder not found", "open_in_explorer")
        })?
    };

    run_blocking("open_in_explorer", move || open_in_windows_explorer(&path)).await
}

pub fn load_settings(state: &AppState) -> Result<AppSettings, ExplorerError> {
    state.settings_store.load_settings()
}

pub fn save_settings(state: &AppState, settings: &AppSettings) -> Result<(), ExplorerError> {
    state.settings_store.save_settings(settings)
}

pub fn add_favorite(state: &AppState, path: &str) -> Result<(), ExplorerError> {
    state.settings_store.add_favorite(path)
}

pub fn remove_favorite(state: &AppState, path: &str) -> Result<(), ExplorerError> {
    state.settings_store.remove_favorite(path)
}

pub async fn plan_create_folder(
    state: &AppState,
    folder_token: &FolderToken,
    name: &str,
) -> Result<OperationPlan, ExplorerError> {
    let parent = state
        .folder_service
        .get_folder_path(folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                "Folder token not found",
                "plan_create_folder",
            )
        })?;

    let operation_service = state.operation_service.clone();
    let n = name.to_string();
    run_blocking("plan_create_folder", move || {
        operation_service.plan_create_folder(&parent, &n)
    })
    .await
}

pub async fn plan_rename(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: &ItemToken,
    new_name: &str,
) -> Result<OperationPlan, ExplorerError> {
    let source = state
        .folder_service
        .resolve_item(folder_token, item_token)
        .ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item token not found", "plan_rename")
        })?;

    let operation_service = state.operation_service.clone();
    let nn = new_name.to_string();
    run_blocking("plan_rename", move || {
        operation_service.plan_rename(&source, &nn)
    })
    .await
}

pub async fn commit_plan(state: &AppState, plan_id: &PlanId) -> Result<JobSummary, ExplorerError> {
    let operation_service = state.operation_service.clone();
    let pid = plan_id.clone();
    run_blocking("commit_plan", move || operation_service.commit_plan(&pid)).await
}

pub async fn execute_create_folder(
    state: &AppState,
    folder_token: &FolderToken,
    name: &str,
) -> Result<JobSummary, ExplorerError> {
    let parent = state
        .folder_service
        .get_folder_path(folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::NotFound,
                "Folder token not found",
                "create_folder",
            )
        })?;

    let operation_service = state.operation_service.clone();
    let n = name.to_string();
    run_blocking("create_folder", move || {
        operation_service.execute_create_folder(&parent, &n)
    })
    .await
}

pub async fn create_folder(
    state: &AppState,
    folder_token: &FolderToken,
    name: &str,
) -> Result<JobSummary, ExplorerError> {
    execute_create_folder(state, folder_token, name).await
}

pub async fn execute_rename(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: &ItemToken,
    new_name: &str,
) -> Result<JobSummary, ExplorerError> {
    let source = state
        .folder_service
        .resolve_item(folder_token, item_token)
        .ok_or_else(|| {
            ExplorerError::new(ErrorCode::NotFound, "Item token not found", "rename_item")
        })?;

    let operation_service = state.operation_service.clone();
    let nn = new_name.to_string();
    run_blocking("rename_item", move || {
        operation_service.execute_rename(&source, &nn)
    })
    .await
}

pub async fn rename_item(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: &ItemToken,
    new_name: &str,
) -> Result<JobSummary, ExplorerError> {
    execute_rename(state, folder_token, item_token, new_name).await
}

pub fn list_jobs(state: &AppState, limit: Option<usize>) -> Result<Vec<JobSummary>, ExplorerError> {
    state
        .operation_service
        .list_recent_jobs(limit.unwrap_or(20))
}

pub async fn plan_paste(
    state: &AppState,
    destination_folder_token: &FolderToken,
) -> Result<Option<OperationPlan>, ExplorerError> {
    let folder_service = state.folder_service.clone();
    let operation_service = state.operation_service.clone();
    let dft = destination_folder_token.clone();

    run_blocking("plan_paste", move || {
        let Some((source_paths, is_cut)) = explorer_win::clipboard::read_clipboard_hdrop()? else {
            return Ok(None);
        };
        let destination = folder_service.get_folder_path(&dft).ok_or_else(|| {
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

pub async fn plan_recycle(
    state: &AppState,
    folder_token: &FolderToken,
    item_tokens: &[ItemToken],
) -> Result<OperationPlan, ExplorerError> {
    let sources = item_tokens
        .iter()
        .map(|token| {
            state
                .folder_service
                .resolve_item(folder_token, token)
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

pub fn resolve_transfer_paths(
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

pub async fn plan_transfer(
    state: &AppState,
    source_folder_token: &FolderToken,
    item_tokens: &[ItemToken],
    destination_folder_token: &FolderToken,
    destination_item_token: Option<&ItemToken>,
    is_move: bool,
) -> Result<OperationPlan, ExplorerError> {
    let folder_service = state.folder_service.clone();
    let operation_service = state.operation_service.clone();
    let sft = source_folder_token.clone();
    let dft = destination_folder_token.clone();
    let tokens = item_tokens.to_vec();
    let dest_item_token = destination_item_token.cloned();

    run_blocking("plan_transfer", move || {
        let (sources, destination) = resolve_transfer_paths(
            &folder_service,
            &sft,
            &tokens,
            &dft,
            dest_item_token.as_ref(),
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

pub async fn read_preview(
    state: &AppState,
    folder_token: &FolderToken,
    item_token: &ItemToken,
) -> Result<PreviewData, ExplorerError> {
    let path = state
        .folder_service
        .resolve_item(folder_token, item_token)
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

pub async fn clipboard_write_items(
    state: &AppState,
    folder_token: &FolderToken,
    item_tokens: &[ItemToken],
    is_cut: bool,
    owner_hwnd: usize,
) -> Result<(), ExplorerError> {
    if owner_hwnd == 0 {
        return Err(ExplorerError::new(
            ErrorCode::AccessDenied,
            "Clipboard owner HWND cannot be NULL",
            "clipboard_write_items",
        ));
    }
    let paths = item_tokens
        .iter()
        .map(|token| {
            state
                .folder_service
                .resolve_item(folder_token, token)
                .ok_or_else(|| {
                    ExplorerError::new(
                        ErrorCode::StaleItem,
                        "Selected item token expired",
                        "clipboard_write_items",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    run_blocking("clipboard_write_items", move || {
        explorer_win::clipboard::write_clipboard_hdrop(&paths, is_cut, owner_hwnd)
    })
    .await
}

pub async fn clipboard_write_path(
    path_utf16: &[u16],
    owner_hwnd: usize,
) -> Result<(), ExplorerError> {
    if owner_hwnd == 0 {
        return Err(ExplorerError::new(
            ErrorCode::AccessDenied,
            "Clipboard owner HWND cannot be NULL",
            "clipboard_write_path",
        ));
    }
    let p = path_utf16.to_vec();
    run_blocking("clipboard_write_path", move || {
        explorer_win::clipboard::write_clipboard_text_utf16(&p, owner_hwnd)
    })
    .await
}

pub fn watch_folder(
    state: &AppState,
    folder_token: &FolderToken,
    subscriber_id: &str,
) -> Result<(), ExplorerError> {
    let p = state
        .folder_service
        .get_folder_path(folder_token)
        .ok_or_else(|| {
            ExplorerError::new(
                ErrorCode::StaleItem,
                "Folder token expired before watch registration",
                "watch_folder",
            )
        })?;
    state
        .watch_service
        .subscribe(&p, explorer_watch::WatchMode::NonRecursive, subscriber_id)
}

pub fn unwatch_all(state: &AppState, subscriber_id: &str) -> Result<(), ExplorerError> {
    state.watch_service.unsubscribe_all(subscriber_id)
}

pub fn list_indexed_roots(
    state: &AppState,
) -> Result<Vec<explorer_index::IndexedRoot>, ExplorerError> {
    state.index_service.list_roots()
}

pub async fn add_indexed_root(
    state: &AppState,
    folder_token: &FolderToken,
) -> Result<explorer_index::IndexedRoot, ExplorerError> {
    let p = state
        .folder_service
        .get_folder_path(folder_token)
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

pub async fn remove_indexed_root(state: &AppState, root_id: &str) -> Result<(), ExplorerError> {
    let index_service = state.index_service.clone();
    let watch_service = state.watch_service.clone();
    let rid = root_id.to_string();
    run_blocking("remove_indexed_root", move || {
        let subscriber_id = format!("indexed-root:{rid}");
        watch_service.unsubscribe_all(&subscriber_id)?;
        index_service.remove_root(&rid)
    })
    .await
}

pub async fn recrawl_indexed_root(state: &AppState, root_id: &str) -> Result<(), ExplorerError> {
    let index_service = state.index_service.clone();
    let rid = root_id.to_string();
    run_blocking("recrawl_indexed_root", move || {
        index_service.recrawl_root(&rid)
    })
    .await
}

pub fn search_indexed(
    state: &AppState,
    query: &str,
    root_id: Option<&str>,
    page: usize,
    page_size: usize,
) -> Result<explorer_index::SearchResponse, ExplorerError> {
    state.index_service.search(query, root_id, page, page_size)
}

pub async fn open_path(
    state: &AppState,
    path_utf16: &[u16],
) -> Result<Option<NavigationResponse>, ExplorerError> {
    open_path_with(state, path_utf16, true).await
}

pub async fn open_path_with(
    state: &AppState,
    path_utf16: &[u16],
    include_hidden: bool,
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
        let nav = run_blocking("open_path", move || {
            folder_svc.navigate_with(&p, include_hidden)
        })
        .await?;
        Ok(Some(nav))
    } else {
        run_blocking("open_path", move || open_file_with_association(&p)).await?;
        Ok(None)
    }
}

pub fn path_from_utf16(
    path_utf16: &[u16],
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
    let p = PathBuf::from(std::ffi::OsString::from_wide(path_utf16));
    Ok(explorer_win::path::normalize_drive_root(&p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_from_utf16_validation() {
        assert!(path_from_utf16(&[], "test").is_err());
        assert!(path_from_utf16(&[b'C' as u16, b':' as u16, 0], "test").is_err());
        let valid: Vec<u16> = "C:\\Windows".encode_utf16().collect();
        let p = path_from_utf16(&valid, "test").unwrap();
        assert_eq!(p, PathBuf::from("C:\\Windows"));
    }

    #[tokio::test]
    async fn test_clipboard_write_rejects_null_owner() {
        let err = clipboard_write_path(&[b'C' as u16], 0).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::AccessDenied);
    }
}
