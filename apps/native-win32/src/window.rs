#![allow(unsafe_op_in_unsafe_fn, dead_code)]

use crate::controls::*;
use crate::dialogs::{
    confirm_recycle, prompt_input, refuse_permanent_delete, show_activity_dialog, show_error,
    show_info, show_roots_manager_dialog,
};
use crate::dispatcher::{SharedDispatcher, UiEvent, WM_APP_WAKEUP};
use crate::presenter::{PAGE_SIZE, TabPresenter};
use crate::preview::PreviewPane;
use crate::resources::{SafeBrush, SafeFont};
use crate::sidebar::SidebarManager;
use explorer_app::requests::{CompletionEnvelope, PageRequest, RequestIdentity};
use explorer_app::state::AppState;
use explorer_domain::ids::{ItemToken, SessionId};
use explorer_domain::models::{EntryKind, SortColumn, SortDirection};
use explorer_win::known_folders::{get_logical_drives, get_standard_known_folders};
use std::path::PathBuf;
use std::sync::Arc;
use windows::Win32::Foundation::{BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, PWSTR, w};

const CLASS_NAME: PCWSTR = w!("RustExplorerNativeMainWindow");

pub struct WindowState {
    pub hwnd_main: HWND,
    pub hwnd_btn_back: HWND,
    pub hwnd_btn_forward: HWND,
    pub hwnd_btn_up: HWND,
    pub hwnd_btn_refresh: HWND,
    pub hwnd_address: HWND,
    pub hwnd_search: HWND,
    pub hwnd_btn_theme: HWND,
    pub hwnd_btn_preview: HWND,
    pub hwnd_tabs: HWND,
    pub hwnd_sidebar: HWND,
    pub hwnd_list: HWND,
    pub hwnd_status: HWND,
    pub preview_pane: Option<PreviewPane>,
    pub sidebar_manager: Option<SidebarManager>,
    pub app_state: Arc<AppState>,
    pub dispatcher: SharedDispatcher,
    pub runtime_handle: tokio::runtime::Handle,
    pub session_id: SessionId,
    pub tabs: Vec<TabPresenter>,
    pub active_tab_idx: usize,
    pub dpi: u32,
    pub font: Option<SafeFont>,
    /// Segoe MDL2 Assets, used for the toolbar arrow glyphs.
    pub nav_font: Option<SafeFont>,
    pub is_dark_mode: bool,
    pub next_tab_id: u64,
    pub watch_refresh_pending: bool,
    pub icon_cache: std::collections::HashMap<String, i32>,
    pub status_message: Option<String>,
    pub dark_background: Option<SafeBrush>,
    pub dark_control: Option<SafeBrush>,
    /// Error captured while a modal dialog was open; surfaced afterwards.
    pub pending_error: Option<String>,
}

/// `LVM_SETEMPTYTEXT` (not exported by the windows crate bindings).
const LVM_SETEMPTYTEXT: u32 = 0x1000 + 204;

/// Debounce timer for filesystem watch notifications on the active folder.
const WATCH_REFRESH_TIMER: usize = 1;
/// Message lifetime for transient status-bar feedback.
const STATUS_MESSAGE_TIMER: usize = 2;

impl WindowState {
    pub fn active_tab(&self) -> &TabPresenter {
        &self.tabs[self.active_tab_idx]
    }

    pub fn active_tab_mut(&mut self) -> &mut TabPresenter {
        &mut self.tabs[self.active_tab_idx]
    }

    pub fn scale(&self, val: i32) -> i32 {
        (val * self.dpi as i32) / 96
    }

    pub fn request_page(&mut self, offset: usize) {
        let session_id = self.session_id.clone();
        let (tab_id, epoch, generation, sort_column, sort_direction, folder_token) = {
            let tab = self.active_tab_mut();
            if tab.pending_page_requests.contains(&offset) {
                return;
            }
            // Pages are immutable for a given generation/sort: never re-fetch a
            // resident page. Re-requesting on every LVN_ODCACHEHINT repaint is what
            // turns a page completion into an endless invalidate/request feedback loop.
            if tab.cache.contains_page(offset) {
                return;
            }
            let Some(folder_token) = tab.folder_token.clone() else {
                return;
            };
            tab.pending_page_requests.insert(offset);
            (
                tab.tab_id.clone(),
                tab.view_epoch,
                tab.generation,
                tab.sort_column,
                tab.sort_direction,
                folder_token,
            )
        };

        let req = PageRequest {
            identity: RequestIdentity::new(session_id, tab_id, 0, epoch),
            folder_token,
            generation,
            offset,
            limit: PAGE_SIZE,
            sort_column,
            sort_direction,
        };

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res = explorer_app::operations::list_page(
                &app_state,
                &req.folder_token,
                req.generation,
                req.offset,
                req.limit,
                Some(req.sort_column),
                Some(req.sort_direction),
            )
            .await;
            dispatcher.enqueue(UiEvent::DirectoryPageReady(CompletionEnvelope {
                identity: req.identity,
                result: res,
            }));
        });
    }

    pub fn trigger_navigate(&mut self, target: PathBuf) {
        let session_id = self.session_id.clone();
        let include_hidden = self.active_tab().show_hidden;
        let (tab_id, epoch) = {
            let tab = self.active_tab_mut();
            let epoch = tab.navigate_to(target.clone());
            (tab.tab_id.clone(), epoch)
        };

        let req_id = RequestIdentity::new(session_id, tab_id, 0, epoch);
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res =
                explorer_app::operations::navigate_with(&app_state, &target, include_hidden).await;
            dispatcher.enqueue(UiEvent::NavigationReady(CompletionEnvelope {
                identity: req_id,
                result: res,
            }));
        });
    }

    pub fn trigger_navigate_history(&mut self, target: PathBuf) {
        let session_id = self.session_id.clone();
        let include_hidden = self.active_tab().show_hidden;
        let (tab_id, epoch) = {
            let tab = self.active_tab();
            (tab.tab_id.clone(), tab.view_epoch)
        };

        let req_id = RequestIdentity::new(session_id, tab_id, 0, epoch);
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res =
                explorer_app::operations::navigate_with(&app_state, &target, include_hidden).await;
            dispatcher.enqueue(UiEvent::NavigationReady(CompletionEnvelope {
                identity: req_id,
                result: res,
            }));
        });
    }

    pub fn trigger_refresh(&mut self) {
        let session_id = self.session_id.clone();
        let (tab_id, epoch, path, include_hidden) = {
            let tab = self.active_tab_mut();
            tab.view_epoch += 1;
            (
                tab.tab_id.clone(),
                tab.view_epoch,
                tab.current_path.clone(),
                tab.show_hidden,
            )
        };

        let req_id = RequestIdentity::new(session_id, tab_id, 0, epoch);
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res =
                explorer_app::operations::navigate_with(&app_state, &path, include_hidden).await;
            dispatcher.enqueue(UiEvent::RefreshReady(CompletionEnvelope {
                identity: req_id,
                result: res,
            }));
        });
    }

    pub fn trigger_search(&mut self, query: String) {
        let session_id = self.session_id.clone();
        let tab = self.active_tab_mut();
        tab.view_epoch += 1;
        let req_id = RequestIdentity::new(session_id, tab.tab_id.clone(), 0, tab.view_epoch);
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let q = query.clone();
            let res = tokio::task::spawn_blocking(move || {
                explorer_app::operations::search_indexed(&app_state, &q, None, 100, 0)
            })
            .await
            .unwrap_or_else(|_| {
                Err(explorer_domain::errors::ExplorerError::new(
                    explorer_domain::errors::ErrorCode::Internal,
                    "Search task panicked",
                    "search_indexed",
                ))
            });
            dispatcher.enqueue(UiEvent::SearchReady(CompletionEnvelope {
                identity: req_id,
                result: res,
            }));
        });
    }

    pub fn trigger_preview_load(&mut self, item_idx: usize) {
        let Some(entry) = self.active_tab().get_visible_entry(item_idx) else {
            return;
        };
        let item_token = entry.token.clone();
        let Some(folder_token) = self.active_tab().folder_token.clone() else {
            return;
        };

        let session_id = self.session_id.clone();
        let tab_id = self.active_tab().tab_id.clone();
        let epoch = self.active_tab().view_epoch;
        let req_id = RequestIdentity::new(session_id, tab_id, 0, epoch);
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res =
                explorer_app::operations::read_preview(&app_state, &folder_token, &item_token)
                    .await;
            dispatcher.enqueue(UiEvent::PreviewReady(CompletionEnvelope {
                identity: req_id,
                result: res,
            }));
        });
    }

    pub fn create_new_tab(&mut self, path: PathBuf) {
        let tab_id = format!("tab-{}", self.next_tab_id);
        self.next_tab_id += 1;
        let new_tab = TabPresenter::new(tab_id, path.clone());
        let title = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| path.to_str().unwrap_or("Folder"));
        let mut title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let tab_idx = self.tabs.len();
        self.tabs.push(new_tab);

        let mut item = TCITEMW {
            mask: TCIF_TEXT,
            pszText: PWSTR(title_utf16.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tabs,
                TCM_INSERTITEMW,
                WPARAM(tab_idx),
                LPARAM(&mut item as *mut _ as isize),
            );
            let _ = SendMessageW(self.hwnd_tabs, TCM_SETCURSEL, WPARAM(tab_idx), LPARAM(0));
        }
        self.active_tab_idx = tab_idx;
        self.trigger_navigate(path);
    }

    pub fn close_current_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        let cur_idx = self.active_tab_idx;
        let closed_tab_id = self.tabs[cur_idx].tab_id.clone();
        let _ = explorer_app::operations::unwatch_all(&self.app_state, &closed_tab_id);
        self.tabs.remove(cur_idx);
        unsafe {
            let _ = SendMessageW(self.hwnd_tabs, TCM_DELETEITEM, WPARAM(cur_idx), LPARAM(0));
        }
        self.active_tab_idx = if cur_idx >= self.tabs.len() {
            self.tabs.len() - 1
        } else {
            cur_idx
        };
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tabs,
                TCM_SETCURSEL,
                WPARAM(self.active_tab_idx),
                LPARAM(0),
            );
        }
        let count = self.active_tab().visible_item_count();
        unsafe {
            let _ = SendMessageW(self.hwnd_list, LVM_SETITEMCOUNT, WPARAM(count), LPARAM(1));
            let _ = InvalidateRect(self.hwnd_list, None, true);
        }
        self.update_address_bar();
        self.update_status_bar();
    }

    pub fn switch_tab(&mut self, new_idx: usize) {
        if new_idx < self.tabs.len() && new_idx != self.active_tab_idx {
            self.active_tab_idx = new_idx;
            unsafe {
                let _ = SendMessageW(self.hwnd_tabs, TCM_SETCURSEL, WPARAM(new_idx), LPARAM(0));
            }
            let count = self.active_tab().visible_item_count();
            unsafe {
                let _ = SendMessageW(self.hwnd_list, LVM_SETITEMCOUNT, WPARAM(count), LPARAM(1));
                let _ = InvalidateRect(self.hwnd_list, None, true);
            }
            self.update_address_bar();
            self.update_status_bar();
        }
    }

    pub fn update_tab_title(&mut self) {
        let display = explorer_win::path::to_display_string(&self.active_tab().current_path);
        let title = std::path::Path::new(&display)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if display.is_empty() {
                    "Folder".to_string()
                } else {
                    display.trim_end_matches('\\').to_string()
                }
            });
        let mut title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let mut item = TCITEMW {
            mask: TCIF_TEXT,
            pszText: PWSTR(title_utf16.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            let _ = SendMessageW(
                self.hwnd_tabs,
                TCM_SETITEMW,
                WPARAM(self.active_tab_idx),
                LPARAM(&mut item as *mut _ as isize),
            );
        }
    }

    pub fn action_select_all(&mut self) {
        let count = self.active_tab().visible_item_count();
        let tab = self.active_tab_mut();
        for i in 0..count {
            tab.selected_indices.insert(i);
        }
        unsafe {
            let mut item = LVITEMW {
                state: LVIS_SELECTED,
                stateMask: LVIS_SELECTED,
                ..Default::default()
            };
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETITEMSTATE,
                WPARAM(usize::MAX),
                LPARAM(&mut item as *mut _ as isize),
            );
            let _ = InvalidateRect(self.hwnd_list, None, true);
        }
        self.update_status_bar();
    }

    pub fn action_new_folder(&mut self) {
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };
        let Some(name) = prompt_input(
            self.hwnd_main,
            "New Folder",
            "Enter folder name:",
            "New folder",
        ) else {
            return;
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        let req_id = RequestIdentity::new(
            self.session_id.clone(),
            self.active_tab().tab_id.clone(),
            0,
            self.active_tab().view_epoch,
        );

        self.runtime_handle.spawn(async move {
            match explorer_app::operations::plan_create_folder(&app_state, &ft, &name).await {
                Ok(plan) => {
                    match explorer_app::operations::commit_plan(&app_state, &plan.id).await {
                        Ok(summary) => {
                            dispatcher.enqueue(UiEvent::JobCompleted(CompletionEnvelope {
                                identity: req_id,
                                result: Ok(summary),
                            }));
                        }
                        Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
                    }
                }
                Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
            }
        });
    }

    pub fn action_rename(&mut self) {
        let selected_idx = self.active_tab().selected_indices.iter().copied().next();
        let Some(idx) = selected_idx else { return };
        let Some(entry) = self.active_tab().get_visible_entry(idx) else {
            return;
        };
        let item_token = entry.token.clone();
        let old_name = entry.display_name.clone();
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };

        let Some(new_name) =
            prompt_input(self.hwnd_main, "Rename Item", "Enter new name:", &old_name)
        else {
            return;
        };
        let new_name = new_name.trim().to_string();
        if new_name.is_empty() || new_name == old_name {
            return;
        }

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        let req_id = RequestIdentity::new(
            self.session_id.clone(),
            self.active_tab().tab_id.clone(),
            0,
            self.active_tab().view_epoch,
        );

        self.runtime_handle.spawn(async move {
            match explorer_app::operations::plan_rename(&app_state, &ft, &item_token, &new_name)
                .await
            {
                Ok(plan) => {
                    match explorer_app::operations::commit_plan(&app_state, &plan.id).await {
                        Ok(summary) => {
                            dispatcher.enqueue(UiEvent::JobCompleted(CompletionEnvelope {
                                identity: req_id,
                                result: Ok(summary),
                            }));
                        }
                        Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
                    }
                }
                Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
            }
        });
    }

    pub fn action_recycle(&mut self) {
        let indices: Vec<usize> = self.active_tab().selected_indices.iter().copied().collect();
        if indices.is_empty() {
            return;
        }
        if !confirm_recycle(self.hwnd_main, indices.len()) {
            return;
        }

        let tokens: Vec<ItemToken> = indices
            .iter()
            .filter_map(|&i| {
                self.active_tab()
                    .get_visible_entry(i)
                    .map(|e| e.token.clone())
            })
            .collect();
        if tokens.is_empty() {
            return;
        }

        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        let req_id = RequestIdentity::new(
            self.session_id.clone(),
            self.active_tab().tab_id.clone(),
            0,
            self.active_tab().view_epoch,
        );

        self.runtime_handle.spawn(async move {
            match explorer_app::operations::plan_recycle(&app_state, &ft, &tokens).await {
                Ok(plan) => {
                    match explorer_app::operations::commit_plan(&app_state, &plan.id).await {
                        Ok(summary) => {
                            dispatcher.enqueue(UiEvent::JobCompleted(CompletionEnvelope {
                                identity: req_id,
                                result: Ok(summary),
                            }));
                        }
                        Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
                    }
                }
                Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
            }
        });
    }

    pub fn action_copy(&mut self, is_cut: bool) {
        let indices: Vec<usize> = self.active_tab().selected_indices.iter().copied().collect();
        if indices.is_empty() {
            return;
        }
        let tokens: Vec<ItemToken> = indices
            .iter()
            .filter_map(|&i| {
                self.active_tab()
                    .get_visible_entry(i)
                    .map(|e| e.token.clone())
            })
            .collect();
        if tokens.is_empty() {
            return;
        }
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };

        let app_state = self.app_state.clone();
        let owner = self.hwnd_main.0 as usize;
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res = explorer_app::operations::clipboard_write_items(
                &app_state, &ft, &tokens, is_cut, owner,
            )
            .await;
            if let Err(err) = res {
                dispatcher.enqueue(UiEvent::OperationError(err));
            }
        });
    }

    pub fn action_paste(&mut self) {
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        let req_id = RequestIdentity::new(
            self.session_id.clone(),
            self.active_tab().tab_id.clone(),
            0,
            self.active_tab().view_epoch,
        );

        self.runtime_handle.spawn(async move {
            match explorer_app::operations::plan_paste(&app_state, &ft).await {
                Ok(Some(plan)) => {
                    match explorer_app::operations::commit_plan(&app_state, &plan.id).await {
                        Ok(summary) => {
                            dispatcher.enqueue(UiEvent::JobCompleted(CompletionEnvelope {
                                identity: req_id,
                                result: Ok(summary),
                            }));
                        }
                        Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
                    }
                }
                Ok(None) => {}
                Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
            }
        });
    }

    /// Opens the focused/selected row: folders navigate in-tab, files launch.
    pub fn open_selected_item(&mut self) {
        let selected = unsafe {
            SendMessageW(
                self.hwnd_list,
                LVM_GETNEXTITEM,
                WPARAM(usize::MAX),
                LPARAM(LVNI_SELECTED as isize),
            )
            .0
        };
        if selected < 0 {
            return;
        }
        let Some(entry) = self.active_tab().get_visible_entry(selected as usize) else {
            return;
        };
        let item_token = entry.token.clone();
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };
        let include_hidden = self.active_tab().show_hidden;
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        let req_id = RequestIdentity::new(
            self.session_id.clone(),
            self.active_tab().tab_id.clone(),
            0,
            self.active_tab().view_epoch,
        );

        self.runtime_handle.spawn(async move {
            match explorer_app::operations::open_item_with(
                &app_state,
                &ft,
                &item_token,
                include_hidden,
            )
            .await
            {
                Ok(Some(nav)) => {
                    dispatcher.enqueue(UiEvent::NavigationReady(CompletionEnvelope {
                        identity: req_id,
                        result: Ok(nav),
                    }));
                }
                Ok(None) => {}
                Err(err) => dispatcher.enqueue(UiEvent::OperationError(err)),
            }
        });
    }

    /// Re-sorts the current snapshot without re-enumerating the directory.
    pub fn resort(&mut self) {
        let tab = self.active_tab_mut();
        tab.view_epoch += 1;
        tab.cache.clear();
        tab.pending_page_requests.clear();
        tab.selected_indices.clear();
        let count = tab.visible_item_count();
        unsafe {
            let _ = SendMessageW(self.hwnd_list, LVM_SETITEMCOUNT, WPARAM(count), LPARAM(1));
            let _ = InvalidateRect(self.hwnd_list, None, true);
        }
        self.update_status_bar();
        self.update_sort_indicator();
        self.request_page(0);
    }

    /// Draws the ascending/descending arrow on the active sort column.
    pub fn update_sort_indicator(&self) {
        let column = self.active_tab().sort_column;
        let ascending = self.active_tab().sort_direction == SortDirection::Ascending;
        let index = match column {
            SortColumn::Name => 0,
            SortColumn::Type => 1,
            SortColumn::Size => 2,
            SortColumn::Modified => 3,
        };
        unsafe {
            let header = SendMessageW(self.hwnd_list, LVM_GETHEADER, WPARAM(0), LPARAM(0));
            if header.0 == 0 {
                return;
            }
            let header = HWND(header.0 as *mut _);
            for current in 0..4 {
                let mut item = HDITEMW {
                    mask: HDI_FORMAT,
                    ..Default::default()
                };
                if SendMessageW(
                    header,
                    HDM_GETITEMW,
                    WPARAM(current),
                    LPARAM(&mut item as *mut _ as isize),
                )
                .0 == 0
                {
                    continue;
                }
                let mut format = item.fmt.0 & !(HDF_SORTUP.0 | HDF_SORTDOWN.0);
                if current == index {
                    format |= if ascending {
                        HDF_SORTUP.0
                    } else {
                        HDF_SORTDOWN.0
                    };
                }
                item.mask = HDI_FORMAT;
                item.fmt = HEADER_CONTROL_FORMAT_FLAGS(format);
                let _ = SendMessageW(
                    header,
                    HDM_SETITEMW,
                    WPARAM(current),
                    LPARAM(&mut item as *mut _ as isize),
                );
            }
        }
    }

    /// Handles an address-bar entry: folders navigate, files are launched.
    pub fn navigate_from_address(&mut self, raw: &str) {
        let expanded = expand_environment_variables(raw.trim().trim_matches('"'));
        if expanded.is_empty() {
            self.update_address_bar();
            return;
        }
        let path = PathBuf::from(&expanded);
        if path.is_dir() {
            self.trigger_navigate(path);
            return;
        }
        if !path.exists() {
            show_error(
                self.hwnd_main,
                "Path not found",
                &format!("Windows cannot find\n\n{expanded}\n\nCheck the spelling and try again."),
            );
            self.update_address_bar();
            return;
        }

        let include_hidden = self.active_tab().show_hidden;
        let utf16: Vec<u16> = expanded.encode_utf16().collect();
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        self.runtime_handle.spawn(async move {
            if let Err(err) =
                explorer_app::operations::open_path_with(&app_state, &utf16, include_hidden).await
            {
                dispatcher.enqueue(UiEvent::OperationError(err));
            }
        });
    }

    /// Resolves (and caches) the shell icon index for a visible row.
    pub fn icon_index_for(&mut self, row: usize) -> i32 {
        let Some(entry) = self.active_tab().get_visible_entry(row) else {
            return 0;
        };
        let is_dir = entry.kind == EntryKind::Directory;
        let key = match entry.kind {
            EntryKind::Directory => "dir".to_string(),
            EntryKind::ReparsePoint => "link".to_string(),
            EntryKind::File => {
                if entry.extension.is_empty() {
                    "file".to_string()
                } else {
                    entry.extension.to_ascii_lowercase()
                }
            }
        };
        if let Some(index) = self.icon_cache.get(&key) {
            return *index;
        }
        let index = crate::icons::icon_index(&key, is_dir);
        self.icon_cache.insert(key, index);
        index
    }

    pub fn action_properties(&mut self) {
        let selected_idx = self.active_tab().selected_indices.iter().copied().next();
        let item_token = selected_idx
            .and_then(|idx| self.active_tab().get_visible_entry(idx))
            .map(|e| e.token.clone());
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res =
                explorer_app::operations::show_properties(&app_state, &ft, item_token.as_ref())
                    .await;
            if let Err(err) = res {
                dispatcher.enqueue(UiEvent::OperationError(err));
            }
        });
    }

    pub fn action_open_in_explorer(&mut self) {
        let selected_idx = self.active_tab().selected_indices.iter().copied().next();
        let item_token = selected_idx
            .and_then(|idx| self.active_tab().get_visible_entry(idx))
            .map(|e| e.token.clone());
        let Some(ft) = self.active_tab().folder_token.clone() else {
            return;
        };

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let res =
                explorer_app::operations::open_in_explorer(&app_state, &ft, item_token.as_ref())
                    .await;
            if let Err(err) = res {
                dispatcher.enqueue(UiEvent::OperationError(err));
            }
        });
    }

    pub fn action_toggle_preview(&mut self) {
        if let Some(ref mut pane) = self.preview_pane {
            let new_vis = !pane.visible;
            pane.set_visible(new_vis);
        }
        self.layout_children();
    }

    pub fn action_toggle_hidden(&mut self) {
        let next = !self.active_tab().show_hidden;
        self.active_tab_mut().show_hidden = next;
        self.active_tab_mut().set_filter("");
        // Hidden/system entries are excluded by the snapshot, so re-enumerate.
        self.trigger_refresh();
    }

    pub fn action_toggle_dark_mode(&mut self) {
        self.is_dark_mode = !self.is_dark_mode;
        self.apply_theme();
    }

    /// Applies or removes the dark palette across the shell chrome.
    pub fn apply_theme(&mut self) {
        const TEXT: u32 = 0x00F0F0F0;
        const BACKGROUND: u32 = 0x00202020;
        const PANEL: u32 = 0x002C2C2C;
        const CLR_NONE: u32 = 0xFFFF_FFFF;
        let dark = self.is_dark_mode;

        if dark {
            if self.dark_background.is_none() {
                let brush = unsafe { CreateSolidBrush(COLORREF(BACKGROUND)) };
                self.dark_background = Some(SafeBrush::new(brush));
            }
            if self.dark_control.is_none() {
                let brush = unsafe { CreateSolidBrush(COLORREF(PANEL)) };
                self.dark_control = Some(SafeBrush::new(brush));
            }
        } else {
            self.dark_background = None;
            self.dark_control = None;
        }

        unsafe {
            // Immersive dark title bar (Windows 10 1809+ / Windows 11).
            let enabled = BOOL::from(dark);
            let _ = DwmSetWindowAttribute(
                self.hwnd_main,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &enabled as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<BOOL>() as u32,
            );

            // Native dark control rendering (edit/listview/treeview scrollbars and
            // text) via the undocumented-but-stable dark theme class.
            let theme = if dark {
                w!("DarkMode_Explorer")
            } else {
                w!("Explorer")
            };
            for control in [
                self.hwnd_address,
                self.hwnd_search,
                self.hwnd_list,
                self.hwnd_sidebar,
                self.hwnd_tabs,
                self.hwnd_status,
            ] {
                if !control.is_invalid() {
                    let _ = SetWindowTheme(control, theme, PCWSTR::null());
                }
            }
            if let Some(pane) = self.preview_pane.as_ref() {
                let _ = SetWindowTheme(pane.hwnd_container, theme, PCWSTR::null());
                let _ = SetWindowTheme(pane.hwnd_text, theme, PCWSTR::null());
            }
            let header = SendMessageW(self.hwnd_list, LVM_GETHEADER, WPARAM(0), LPARAM(0));
            if header.0 != 0 {
                let header_theme = if dark {
                    w!("DarkMode_ItemsView")
                } else {
                    w!("ItemsView")
                };
                let _ = SetWindowTheme(HWND(header.0 as *mut _), header_theme, PCWSTR::null());
                let _ = InvalidateRect(HWND(header.0 as *mut _), None, true);
            }

            // Let the dark control theme paint list/tree/status surfaces; forcing
            // LVM/TVM colors on top of a themed control produces dark-on-dark text.
            let _ = InvalidateRect(self.hwnd_main, None, true);
            let _ = InvalidateRect(self.hwnd_list, None, true);
            let _ = InvalidateRect(self.hwnd_sidebar, None, true);
            let _ = InvalidateRect(self.hwnd_status, None, true);
        }
    }

    pub fn action_add_favorite(&mut self) {
        let path_str = self.active_tab().current_path.to_string_lossy().to_string();
        let _ = explorer_app::operations::add_favorite(&self.app_state, &path_str);
        self.refresh_sidebar();
        show_info(
            self.hwnd_main,
            "Favorites",
            &format!("Added {} to Favorites.", path_str),
        );
    }

    pub fn action_remove_favorite(&mut self) {
        let path_str = self.active_tab().current_path.to_string_lossy().to_string();
        let _ = explorer_app::operations::remove_favorite(&self.app_state, &path_str);
        self.refresh_sidebar();
        show_info(
            self.hwnd_main,
            "Favorites",
            &format!("Removed {} from Favorites.", path_str),
        );
    }

    pub fn refresh_sidebar(&mut self) {
        let kf = get_standard_known_folders();
        let dr = get_logical_drives();
        let settings = explorer_app::operations::load_settings(&self.app_state).unwrap_or_default();
        if let Some(ref mut sb) = self.sidebar_manager {
            sb.populate(&kf, &dr, &settings.favorites);
        }
    }

    pub fn update_address_bar(&self) {
        let path_display = explorer_win::path::to_display_string(&self.active_tab().current_path);
        let mut utf16: Vec<u16> = path_display.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let _ = SetWindowTextW(self.hwnd_address, PCWSTR(utf16.as_mut_ptr()));
        }
    }

    pub fn update_status_bar(&self) {
        let total = self.active_tab().total_items;
        let visible = self.active_tab().visible_item_count();
        let sel_count = self.active_tab().selected_indices.len();

        let part1 = explorer_win::path::to_display_string(&self.active_tab().current_path);
        let part2 = if visible != total {
            format!("{} items (filtered from {})", visible, total)
        } else {
            format!("{} items", total)
        };
        let part3 = if let Some(message) = self.status_message.clone() {
            message
        } else if sel_count > 0 {
            format!(
                "{} item{} selected",
                sel_count,
                if sel_count == 1 { "" } else { "s" }
            )
        } else {
            String::new()
        };

        let p1_utf16: Vec<u16> = part1.encode_utf16().chain(Some(0)).collect();
        let p2_utf16: Vec<u16> = part2.encode_utf16().chain(Some(0)).collect();
        let p3_utf16: Vec<u16> = part3.encode_utf16().chain(Some(0)).collect();

        unsafe {
            let _ = SendMessageW(
                self.hwnd_status,
                SB_SETTEXTW,
                WPARAM(0),
                LPARAM(p1_utf16.as_ptr() as isize),
            );
            let _ = SendMessageW(
                self.hwnd_status,
                SB_SETTEXTW,
                WPARAM(1),
                LPARAM(p2_utf16.as_ptr() as isize),
            );
            let _ = SendMessageW(
                self.hwnd_status,
                SB_SETTEXTW,
                WPARAM(2),
                LPARAM(p3_utf16.as_ptr() as isize),
            );
        }
    }

    /// Creates (or recreates, on DPI change) the Segoe UI font and applies it.
    pub fn rebuild_font(&mut self) {
        let font_h = -((10 * self.dpi as i32) / 72);
        let hfont = unsafe {
            CreateFontW(
                font_h,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                DEFAULT_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            )
        };
        if hfont.is_invalid() {
            return;
        }
        let previous = self.font.take();
        self.font = Some(SafeFont::new(hfont));
        self.apply_fonts();
        // The controls now reference the new font; release the previous handle.
        drop(previous);

        // Arrow glyphs come from the MDL2 icon font, not the UI font.
        let nav_height = -((12 * self.dpi as i32) / 72);
        let nav = unsafe {
            CreateFontW(
                nav_height,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                DEFAULT_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe MDL2 Assets"),
            )
        };
        if !nav.is_invalid() {
            let previous_nav = self.nav_font.take();
            self.nav_font = Some(SafeFont::new(nav));
            let raw = self
                .nav_font
                .as_ref()
                .map(|font| font.handle().0 as usize)
                .unwrap_or(0);
            unsafe {
                for button in [self.hwnd_btn_back, self.hwnd_btn_forward, self.hwnd_btn_up] {
                    if !button.is_invalid() {
                        let _ = SendMessageW(button, WM_SETFONT, WPARAM(raw), LPARAM(1));
                    }
                }
            }
            drop(previous_nav);
        }
    }

    /// Applies the UI font to every native control (including the ListView header).
    pub fn apply_fonts(&self) {
        let Some(font) = self.font.as_ref() else {
            return;
        };
        let raw = font.handle().0 as usize;
        let controls = [
            self.hwnd_btn_back,
            self.hwnd_btn_forward,
            self.hwnd_btn_up,
            self.hwnd_btn_refresh,
            self.hwnd_address,
            self.hwnd_search,
            self.hwnd_btn_theme,
            self.hwnd_btn_preview,
            self.hwnd_tabs,
            self.hwnd_sidebar,
            self.hwnd_list,
            self.hwnd_status,
        ];
        unsafe {
            for control in controls {
                if !control.is_invalid() {
                    let _ = SendMessageW(control, WM_SETFONT, WPARAM(raw), LPARAM(1));
                }
            }
            let header = SendMessageW(self.hwnd_list, LVM_GETHEADER, WPARAM(0), LPARAM(0));
            if header.0 != 0 {
                let _ = SendMessageW(HWND(header.0 as *mut _), WM_SETFONT, WPARAM(raw), LPARAM(1));
            }
        }
        if let Some(pane) = self.preview_pane.as_ref() {
            pane.apply_font(font.handle());
        }
    }

    /// Shows transient feedback in the status bar instead of a modal message box.
    pub fn flash_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.update_status_bar();
        unsafe {
            let _ = SetTimer(self.hwnd_main, STATUS_MESSAGE_TIMER, 5000, None);
        }
    }

    pub fn clear_status_message(&mut self) {
        if self.status_message.take().is_some() {
            self.update_status_bar();
        }
    }

    /// Watches the active folder so external changes refresh the view.
    pub fn subscribe_active_folder(&self) {
        let Some(folder_token) = self.active_tab().folder_token.clone() else {
            return;
        };
        let tab_id = self.active_tab().tab_id.clone();
        if let Err(error) =
            explorer_app::operations::watch_folder(&self.app_state, &folder_token, &tab_id)
        {
            tracing::warn!("Unable to watch active folder: {error}");
        }
    }

    pub fn layout_children(&self) {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd_main, &mut rc);
        }
        let width = (rc.right - rc.left).max(1);
        let height = (rc.bottom - rc.top).max(1);

        let scale = |v: i32| (v * self.dpi as i32) / 96;

        let pad = scale(6);
        let gap = scale(4);
        let btn_size = scale(32);
        let tabs_h = scale(26);
        let status_h = scale(23);

        // 1. Navigation buttons
        let mut x = pad;
        unsafe {
            let _ = MoveWindow(self.hwnd_btn_back, x, pad, btn_size, btn_size, true);
            x += btn_size + gap;
            let _ = MoveWindow(self.hwnd_btn_forward, x, pad, btn_size, btn_size, true);
            x += btn_size + gap;
            let _ = MoveWindow(self.hwnd_btn_up, x, pad, btn_size, btn_size, true);
            x += btn_size + gap * 2;
            let refresh_w = scale(76);
            let _ = MoveWindow(self.hwnd_btn_refresh, x, pad, refresh_w, btn_size, true);
            x += refresh_w + gap * 2;
        }

        // 2. Right-aligned tools, then the address fills the remaining space.
        let preview_btn_w = scale(82);
        let theme_btn_w = scale(70);
        let search_w = scale(220);
        let right = width - pad;
        let preview_x = right - preview_btn_w;
        let theme_x = preview_x - gap - theme_btn_w;
        let search_x = theme_x - gap - search_w;
        let address_w = (search_x - gap - x).max(scale(160));

        unsafe {
            let _ = MoveWindow(
                self.hwnd_address,
                x,
                pad + scale(1),
                address_w,
                btn_size - scale(2),
                true,
            );
            let _ = MoveWindow(
                self.hwnd_search,
                search_x,
                pad + scale(1),
                search_w,
                btn_size - scale(2),
                true,
            );
            let _ = MoveWindow(
                self.hwnd_btn_theme,
                theme_x,
                pad,
                theme_btn_w,
                btn_size,
                true,
            );
            let _ = MoveWindow(
                self.hwnd_btn_preview,
                preview_x,
                pad,
                preview_btn_w,
                btn_size,
                true,
            );
        }

        // 3. Tab strip
        let tabs_y = pad + btn_size + gap;
        unsafe {
            let _ = MoveWindow(self.hwnd_tabs, 0, tabs_y, width, tabs_h, true);
        }

        // 4. Sidebar / list / preview
        let content_y = tabs_y + tabs_h;
        let content_h = (height - content_y - status_h).max(scale(50));
        let sidebar_w = scale(240);
        let preview_w = match self.preview_pane.as_ref() {
            Some(pane) if pane.visible => scale(300),
            _ => 0,
        };
        let list_x = sidebar_w;
        let list_w = (width - sidebar_w - preview_w).max(scale(120));

        unsafe {
            let _ = MoveWindow(self.hwnd_sidebar, 0, content_y, sidebar_w, content_h, true);
            let _ = MoveWindow(self.hwnd_list, list_x, content_y, list_w, content_h, true);
        }
        if let Some(ref pane) = self.preview_pane
            && pane.visible
        {
            pane.set_bounds(list_x + list_w, content_y, preview_w, content_h);
        }

        // Stretch the Name column so the report fills the view, leaving room for the
        // vertical scrollbar the control reserves.
        let type_w = scale(96);
        let size_w = scale(96);
        let date_w = scale(160);
        let scrollbar_w = unsafe { GetSystemMetrics(SM_CXVSCROLL) };
        let name_w = (list_w - type_w - size_w - date_w - scale(4) - scrollbar_w).max(scale(180));
        for (column, column_width) in [name_w, type_w, size_w, date_w].into_iter().enumerate() {
            unsafe {
                let _ = SendMessageW(
                    self.hwnd_list,
                    LVM_SETCOLUMNWIDTH,
                    WPARAM(column),
                    LPARAM(column_width as isize),
                );
            }
        }

        // 5. Status bar
        unsafe {
            let _ = MoveWindow(
                self.hwnd_status,
                0,
                height - status_h,
                width,
                status_h,
                true,
            );
            let parts = [width - scale(300), width - scale(150), -1];
            let _ = SendMessageW(
                self.hwnd_status,
                SB_SETPARTS,
                WPARAM(parts.len()),
                LPARAM(parts.as_ptr() as isize),
            );
        }
    }
}

// Subclass for Edit controls (Enter and Escape routing)
unsafe extern "system" fn address_edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    refdata: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN {
        if wparam.0 == 0x0D {
            // Enter: navigate to path
            let state_ptr = refdata as *mut WindowState;
            if let Some(state) = state_ptr.as_mut() {
                let len = GetWindowTextLengthW(hwnd) as usize;
                let mut buf = vec![0u16; len + 1];
                let copied = GetWindowTextW(hwnd, &mut buf) as usize;
                let path_str = String::from_utf16_lossy(&buf[..copied]);
                state.navigate_from_address(&path_str);
                return LRESULT(0);
            }
        } else if wparam.0 == 0x1B {
            // Escape: restore current path
            let state_ptr = refdata as *mut WindowState;
            if let Some(state) = state_ptr.as_mut() {
                state.update_address_bar();
                return LRESULT(0);
            }
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn search_edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    refdata: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN {
        if wparam.0 == 0x0D {
            // Enter: trigger indexed search
            let state_ptr = refdata as *mut WindowState;
            if let Some(state) = state_ptr.as_mut() {
                let len = GetWindowTextLengthW(hwnd) as usize;
                let mut buf = vec![0u16; len + 1];
                let copied = GetWindowTextW(hwnd, &mut buf) as usize;
                let query = String::from_utf16_lossy(&buf[..copied]);
                state.trigger_search(query.trim().to_string());
                return LRESULT(0);
            }
        } else if wparam.0 == 0x1B {
            // Escape: clear filter
            let state_ptr = refdata as *mut WindowState;
            if let Some(state) = state_ptr.as_mut() {
                let _ = SetWindowTextW(hwnd, w!(""));
                state.active_tab_mut().set_filter("");
                let count = state.active_tab().visible_item_count();
                let _ = SendMessageW(state.hwnd_list, LVM_SETITEMCOUNT, WPARAM(count), LPARAM(1));
                let _ = InvalidateRect(state.hwnd_list, None, true);
                state.update_status_bar();
                return LRESULT(0);
            }
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

// Main Window Procedure
/// Panic-proof wrapper: a Rust panic inside a window procedure would otherwise
/// abort the whole process across the `extern "system"` boundary.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wnd_proc_inner(hwnd, msg, wparam, lparam)
    })) {
        Ok(result) => result,
        Err(_) => {
            crate::diagnostics::write_line(&format!(
                "[panic-window-proc] msg={msg:#06x} wp={:#x}",
                wparam.0
            ));
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

unsafe extern "system" fn wnd_proc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if crate::diagnostics::trace_enabled() {
        let mut detail = String::new();
        if msg == WM_NOTIFY && lparam.0 != 0 {
            let header = &*(lparam.0 as *const NMHDR);
            detail = format!("notify={:#x} from={:?}", header.code, header.hwndFrom);
        } else if msg == WM_COMMAND {
            detail = format!(
                "cmd={} code={}",
                wparam.0 & 0xffff,
                (wparam.0 >> 16) & 0xffff
            );
        }
        crate::diagnostics::trace_message(msg, wparam.0, lparam.0, &detail);
    }
    // A modal dialog (or a popup menu) pumps messages on this same thread. Re-entering
    // the state machine here would create a second mutable view of WindowState, so
    // defer all stateful work until the modal loop unwinds.
    if crate::dialogs::in_modal() && matches!(msg, WM_COMMAND | WM_CONTEXTMENU | WM_CLOSE) {
        // Only the entry points that could start a second modal loop (or tear the
        // window down underneath one) are suppressed. Dispatcher/timer/notification
        // traffic must keep flowing or the coalesced wakeup flag would strand events.
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    if !state_ptr.is_null()
        && !crate::dialogs::in_modal()
        && let Some(state) = state_ptr.as_mut()
        && let Some(message) = state.pending_error.take()
    {
        // Never open a message box from inside an arbitrary message (paint/notify);
        // surface the deferred failure through the status bar and the crash log.
        crate::diagnostics::write_line(&format!("[deferred-error] {message}"));
        state.flash_status(message);
    }

    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let state = Box::from_raw(cs.lpCreateParams as *mut WindowState);
            let state_raw = Box::into_raw(state);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_raw as isize);
            let state = &mut *state_raw;
            state.hwnd_main = hwnd;

            let hinst = cs.hInstance;

            // 1. Navigation toolbar buttons
            state.hwnd_btn_back = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("\u{E72B}"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_NAV_BACK as *mut _),
                hinst,
                None,
            )
            .unwrap();

            state.hwnd_btn_forward = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("\u{E72A}"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_NAV_FORWARD as *mut _),
                hinst,
                None,
            )
            .unwrap();

            state.hwnd_btn_up = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("\u{E74A}"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_NAV_UP as *mut _),
                hinst,
                None,
            )
            .unwrap();

            state.hwnd_btn_refresh = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Refresh"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_NAV_REFRESH as *mut _),
                hinst,
                None,
            )
            .unwrap();

            // 2. Address edit control
            state.hwnd_address = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_EDIT_ADDRESS as *mut _),
                hinst,
                None,
            )
            .unwrap();

            let _ = SetWindowSubclass(
                state.hwnd_address,
                Some(address_edit_subclass_proc),
                1,
                state_raw as usize,
            );

            // 3. Search / Filter edit control
            state.hwnd_search = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_EDIT_SEARCH as *mut _),
                hinst,
                None,
            )
            .unwrap();

            let _ = SetWindowSubclass(
                state.hwnd_search,
                Some(search_edit_subclass_proc),
                2,
                state_raw as usize,
            );

            let _ = SendMessageW(
                state.hwnd_search,
                EM_SETCUEBANNER,
                WPARAM(0),
                LPARAM(w!("Search this folder (Ctrl+F)").0 as isize),
            );

            // 4. Theme & Preview toggle buttons
            state.hwnd_btn_theme = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Theme"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_BTN_THEME as *mut _),
                hinst,
                None,
            )
            .unwrap();

            state.hwnd_btn_preview = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!("Preview"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_BTN_PREVIEW as *mut _),
                hinst,
                None,
            )
            .unwrap();

            // 5. Tabs control
            state.hwnd_tabs = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                WC_TABCONTROLW,
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_TAB_CONTROL as *mut _),
                hinst,
                None,
            )
            .unwrap();

            // 6. Sidebar (TreeView)
            state.hwnd_sidebar = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                WC_TREEVIEWW,
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_CLIPSIBLINGS
                    | WINDOW_STYLE(
                        TVS_HASBUTTONS
                            | TVS_SHOWSELALWAYS
                            | TVS_FULLROWSELECT
                            | TVS_DISABLEDRAGDROP,
                    ),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_TREE_SIDEBAR as *mut _),
                hinst,
                None,
            )
            .unwrap();

            state.sidebar_manager = Some(SidebarManager::new(state.hwnd_sidebar));
            state.refresh_sidebar();

            // 7. Virtual ListView
            state.hwnd_list = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                WC_LISTVIEWW,
                PCWSTR::null(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_CLIPSIBLINGS
                    | WINDOW_STYLE(LVS_REPORT | LVS_OWNERDATA | LVS_SHOWSELALWAYS),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_LIST_VIEW as *mut _),
                hinst,
                None,
            )
            .unwrap();

            let _ = SendMessageW(
                state.hwnd_list,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                WPARAM(0),
                LPARAM((LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_GRIDLINES) as isize),
            );

            // Setup ListView columns (Name, Type, Size, Date modified)
            let cols = [
                ("Name", state.scale(260)),
                ("Type", state.scale(90)),
                ("Size", state.scale(90)),
                ("Date modified", state.scale(150)),
            ];
            for (idx, (header, col_w)) in cols.iter().enumerate() {
                let mut header_utf16: Vec<u16> = header.encode_utf16().chain(Some(0)).collect();
                let mut col = LVCOLUMNW {
                    mask: LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM,
                    cx: *col_w,
                    pszText: PWSTR(header_utf16.as_mut_ptr()),
                    iSubItem: idx as i32,
                    ..Default::default()
                };
                let _ = SendMessageW(
                    state.hwnd_list,
                    LVM_INSERTCOLUMNW,
                    WPARAM(idx),
                    LPARAM(&mut col as *mut _ as isize),
                );
            }

            // Real shell icons + a friendly empty state.
            let image_list = crate::icons::system_small_image_list();
            if image_list != 0 {
                let _ = SendMessageW(
                    state.hwnd_list,
                    LVM_SETIMAGELIST,
                    WPARAM(LVSIL_SMALL as usize),
                    LPARAM(image_list),
                );
            }
            let empty_text: Vec<u16> = "This folder is empty."
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let _ = SendMessageW(
                state.hwnd_list,
                LVM_SETEMPTYTEXT,
                WPARAM(0),
                LPARAM(empty_text.as_ptr() as isize),
            );

            // 8. Preview Pane
            state.preview_pane = Some(PreviewPane::new(hwnd, hinst));

            // 9. Status Bar
            state.hwnd_status = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                STATUSCLASSNAMEW,
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(SBARS_SIZEGRIP),
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_STATUS_BAR as *mut _),
                hinst,
                None,
            )
            .unwrap();

            // 10. Initial Tab item
            let init_path = state.active_tab().current_path.clone();
            let title = init_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_else(|| init_path.to_str().unwrap_or("Folder"));
            let mut title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            let mut tab_item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(title_utf16.as_mut_ptr()),
                ..Default::default()
            };
            let _ = SendMessageW(
                state.hwnd_tabs,
                TCM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut tab_item as *mut _ as isize),
            );

            // Trigger initial navigation once the dispatcher is bound to the real
            // HWND (see create_main_window); doing it here would post completions to
            // a dispatcher that still has a null window and silently drop the first page.
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            if let Some(state) = state_ptr.as_ref()
                && state.is_dark_mode
                && let Some(brush) = state.dark_background.as_ref()
            {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(HDC(wparam.0 as *mut _), &rect, brush.handle());
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORLISTBOX => {
            if let Some(state) = state_ptr.as_ref()
                && state.is_dark_mode
                && let Some(brush) = state.dark_control.as_ref()
            {
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetTextColor(hdc, COLORREF(0x00F0F0F0));
                let _ = SetBkColor(hdc, COLORREF(0x002C2C2C));
                return LRESULT(brush.handle().0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam.0 as *mut MINMAXINFO);
            let dpi = state_ptr
                .as_ref()
                .map(|state| state.dpi as i32)
                .unwrap_or(96);
            info.ptMinTrackSize.x = 800 * dpi / 96;
            info.ptMinTrackSize.y = 520 * dpi / 96;
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let new_dpi = (wparam.0 & 0xffff) as u32;
            let suggested = &*(lparam.0 as *const RECT);
            if let Some(state) = state_ptr.as_mut() {
                state.dpi = new_dpi.max(96);
                state.rebuild_font();
                state.layout_children();
            }
            let width = suggested.right - suggested.left;
            let height = suggested.bottom - suggested.top;
            let _ = SetWindowPos(
                hwnd,
                None,
                suggested.left,
                suggested.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(state) = state_ptr.as_ref() {
                state.layout_children();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as u16;
            let code = ((wparam.0 >> 16) & 0xffff) as u16;

            if let Some(state) = state_ptr.as_mut() {
                // Live in-folder filtering on EN_CHANGE
                if id == IDC_EDIT_SEARCH && code as u32 == EN_CHANGE {
                    let len = GetWindowTextLengthW(state.hwnd_search) as usize;
                    let mut buf = vec![0u16; len + 1];
                    let copied = GetWindowTextW(state.hwnd_search, &mut buf) as usize;
                    let query = String::from_utf16_lossy(&buf[..copied]);
                    state.active_tab_mut().set_filter(&query);
                    let count = state.active_tab().visible_item_count();
                    let _ =
                        SendMessageW(state.hwnd_list, LVM_SETITEMCOUNT, WPARAM(count), LPARAM(1));
                    let _ = InvalidateRect(state.hwnd_list, None, true);
                    state.update_status_bar();
                    return LRESULT(0);
                }

                match id {
                    IDC_NAV_BACK => {
                        if let Some(prev) = state.active_tab_mut().go_back() {
                            state.trigger_navigate_history(prev);
                        }
                    }
                    IDC_NAV_FORWARD => {
                        if let Some(next) = state.active_tab_mut().go_forward() {
                            state.trigger_navigate_history(next);
                        }
                    }
                    IDC_NAV_UP => {
                        if let Some(parent) = state.active_tab_mut().go_up() {
                            state.trigger_navigate(parent);
                        }
                    }
                    IDC_NAV_REFRESH => {
                        state.trigger_refresh();
                    }
                    IDC_BTN_THEME | ID_ACTION_TOGGLE_DARK_MODE => {
                        state.action_toggle_dark_mode();
                    }
                    IDC_BTN_PREVIEW | ID_ACTION_TOGGLE_PREVIEW => {
                        state.action_toggle_preview();
                    }
                    ID_ACTION_FOCUS_ADDRESS => {
                        let _ = SetFocus(state.hwnd_address);
                        let _ = SendMessageW(state.hwnd_address, EM_SETSEL, WPARAM(0), LPARAM(-1));
                    }
                    ID_ACTION_FOCUS_SEARCH => {
                        let _ = SetFocus(state.hwnd_search);
                        let _ = SendMessageW(state.hwnd_search, EM_SETSEL, WPARAM(0), LPARAM(-1));
                    }
                    ID_ACTION_NEW_TAB => {
                        let p = state.active_tab().current_path.clone();
                        state.create_new_tab(p);
                    }
                    ID_ACTION_CLOSE_TAB => {
                        state.close_current_tab();
                    }
                    ID_ACTION_NEXT_TAB => {
                        let next = (state.active_tab_idx + 1) % state.tabs.len();
                        state.switch_tab(next);
                    }
                    ID_ACTION_PREV_TAB => {
                        let prev = if state.active_tab_idx == 0 {
                            state.tabs.len() - 1
                        } else {
                            state.active_tab_idx - 1
                        };
                        state.switch_tab(prev);
                    }
                    ID_ACTION_SELECT_ALL => {
                        state.action_select_all();
                    }
                    ID_ACTION_NEW_FOLDER => {
                        state.action_new_folder();
                    }
                    ID_ACTION_RENAME => {
                        state.action_rename();
                    }
                    ID_ACTION_RECYCLE => {
                        state.action_recycle();
                    }
                    ID_ACTION_PERMANENT_DELETE_REFUSAL => {
                        refuse_permanent_delete(state.hwnd_main);
                    }
                    ID_ACTION_COPY => {
                        state.action_copy(false);
                    }
                    ID_ACTION_CUT => {
                        state.action_copy(true);
                    }
                    ID_ACTION_PASTE => {
                        state.action_paste();
                    }
                    ID_ACTION_PROPERTIES => {
                        state.action_properties();
                    }
                    ID_ACTION_OPEN => {
                        state.open_selected_item();
                    }
                    ID_ACTION_OPEN_EXPLORER => {
                        state.action_open_in_explorer();
                    }
                    ID_ACTION_TOGGLE_HIDDEN => {
                        state.action_toggle_hidden();
                    }
                    ID_ACTION_ROOTS_MANAGER => {
                        let token = state.active_tab().folder_token.clone();
                        show_roots_manager_dialog(
                            state.hwnd_main,
                            state.app_state.clone(),
                            state.runtime_handle.clone(),
                            token,
                        );
                    }
                    ID_ACTION_ACTIVITY_LOG => {
                        show_activity_dialog(state.hwnd_main, state.app_state.clone());
                    }
                    ID_ACTION_ADD_FAVORITE => {
                        state.action_add_favorite();
                    }
                    ID_ACTION_REMOVE_FAVORITE => {
                        state.action_remove_favorite();
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let nmhdr = &*(lparam.0 as *const NMHDR);
            if let Some(state) = state_ptr.as_mut() {
                // Tab Selection Change
                if nmhdr.hwndFrom == state.hwnd_tabs && nmhdr.code == TCN_SELCHANGE {
                    let cur_idx = SendMessageW(state.hwnd_tabs, TCM_GETCURSEL, WPARAM(0), LPARAM(0))
                        .0 as usize;
                    state.switch_tab(cur_idx);
                    return LRESULT(0);
                }

                // Sidebar TreeView Selection Change
                if nmhdr.hwndFrom == state.hwnd_sidebar && nmhdr.code == TVN_SELCHANGEDW {
                    let nmtv = &*(lparam.0 as *const NMTREEVIEWW);
                    let hitem = nmtv.itemNew.hItem;
                    if let Some(ref sb) = state.sidebar_manager
                        && let Some(path) = sb.get_path_for_item(hitem)
                    {
                        state.trigger_navigate(path);
                    }
                    return LRESULT(0);
                }

                // ListView Notifications
                if nmhdr.hwndFrom == state.hwnd_list {
                    match nmhdr.code {
                        LVN_GETDISPINFOW => {
                            let plvdi = &mut *(lparam.0 as *mut NMLVDISPINFOW);
                            let row_idx = plvdi.item.iItem as usize;
                            let sub_idx = plvdi.item.iSubItem;

                            if (plvdi.item.mask.0 & LVIF_IMAGE.0) != 0 {
                                plvdi.item.iImage = state.icon_index_for(row_idx);
                            }

                            if (plvdi.item.mask.0 & LVIF_TEXT.0) != 0 {
                                if let Some(entry) = state.active_tab().get_visible_entry(row_idx) {
                                    let text = match sub_idx {
                                        0 => entry.display_name.clone(),
                                        1 => match entry.kind {
                                            EntryKind::Directory => "File folder".to_string(),
                                            EntryKind::ReparsePoint => {
                                                "Link / Junction".to_string()
                                            }
                                            EntryKind::File => {
                                                if entry.extension.is_empty() {
                                                    "File".to_string()
                                                } else {
                                                    format!(
                                                        "{} File",
                                                        entry.extension.to_uppercase()
                                                    )
                                                }
                                            }
                                        },
                                        2 => {
                                            if let Some(bytes) = entry.size_bytes {
                                                if bytes >= 1024 * 1024 {
                                                    format!(
                                                        "{:.2} MB",
                                                        bytes as f64 / (1024.0 * 1024.0)
                                                    )
                                                } else if bytes >= 1024 {
                                                    format!("{:.1} KB", bytes as f64 / 1024.0)
                                                } else {
                                                    format!("{} B", bytes)
                                                }
                                            } else {
                                                String::new()
                                            }
                                        }
                                        3 => {
                                            if let Some(ft) = entry.modified_filetime {
                                                // Windows FileTime to YYYY-MM-DD HH:MM
                                                let st = filetime_to_systemtime(ft);
                                                format!(
                                                    "{:04}-{:02}-{:02} {:02}:{:02}",
                                                    st.wYear,
                                                    st.wMonth,
                                                    st.wDay,
                                                    st.wHour,
                                                    st.wMinute
                                                )
                                            } else {
                                                String::new()
                                            }
                                        }
                                        _ => String::new(),
                                    };

                                    let text_utf16: Vec<u16> =
                                        text.encode_utf16().chain(Some(0)).collect();
                                    let max_len =
                                        (plvdi.item.cchTextMax as usize).min(text_utf16.len());
                                    if !plvdi.item.pszText.is_null() && max_len > 0 {
                                        std::ptr::copy_nonoverlapping(
                                            text_utf16.as_ptr(),
                                            plvdi.item.pszText.0,
                                            max_len,
                                        );
                                        // Ensure NUL terminated
                                        *plvdi.item.pszText.0.add(max_len - 1) = 0;
                                    }
                                } else {
                                    // Missing page: schedule load
                                    let actual_idx = state.active_tab().get_actual_index(row_idx);
                                    let offset = (actual_idx / PAGE_SIZE) * PAGE_SIZE;
                                    state.request_page(offset);

                                    let loading_text: Vec<u16> =
                                        "...".encode_utf16().chain(Some(0)).collect();
                                    let max_len =
                                        (plvdi.item.cchTextMax as usize).min(loading_text.len());
                                    if !plvdi.item.pszText.is_null() && max_len > 0 {
                                        std::ptr::copy_nonoverlapping(
                                            loading_text.as_ptr(),
                                            plvdi.item.pszText.0,
                                            max_len,
                                        );
                                    }
                                }
                            }
                            return LRESULT(0);
                        }
                        LVN_ODCACHEHINT => {
                            let hint = &*(lparam.0 as *const NMLVCACHEHINT);
                            let from_actual =
                                state.active_tab().get_actual_index(hint.iFrom as usize);
                            let to_actual = state.active_tab().get_actual_index(hint.iTo as usize);
                            let offset_from = (from_actual / PAGE_SIZE) * PAGE_SIZE;
                            let offset_to = (to_actual / PAGE_SIZE) * PAGE_SIZE;
                            state.request_page(offset_from);
                            if offset_to != offset_from {
                                state.request_page(offset_to);
                            }
                            return LRESULT(0);
                        }
                        LVN_COLUMNCLICK => {
                            let nmlv = &*(lparam.0 as *const NMLISTVIEW);
                            let new_col = match nmlv.iSubItem {
                                0 => SortColumn::Name,
                                1 => SortColumn::Type,
                                2 => SortColumn::Size,
                                3 => SortColumn::Modified,
                                _ => SortColumn::Name,
                            };
                            let tab = state.active_tab_mut();
                            if tab.sort_column == new_col {
                                tab.sort_direction = match tab.sort_direction {
                                    SortDirection::Ascending => SortDirection::Descending,
                                    SortDirection::Descending => SortDirection::Ascending,
                                };
                            } else {
                                tab.sort_column = new_col;
                                tab.sort_direction = SortDirection::Ascending;
                            }
                            state.resort();
                            return LRESULT(0);
                        }
                        LVN_ITEMCHANGED => {
                            let nmlv = &*(lparam.0 as *const NMLISTVIEW);
                            if (nmlv.uChanged.0 & LVIF_STATE.0) != 0 {
                                let was_sel = (nmlv.uOldState & LVIS_SELECTED.0) != 0;
                                let is_sel = (nmlv.uNewState & LVIS_SELECTED.0) != 0;
                                let idx = nmlv.iItem as usize;
                                if is_sel && !was_sel {
                                    state.active_tab_mut().selected_indices.insert(idx);
                                    state.trigger_preview_load(idx);
                                } else if !is_sel && was_sel {
                                    state.active_tab_mut().selected_indices.remove(&idx);
                                }
                                state.update_status_bar();
                            }
                            return LRESULT(0);
                        }
                        NM_DBLCLK | NM_RETURN => {
                            state.open_selected_item();
                            return LRESULT(0);
                        }
                        NM_RCLICK => {
                            let mut pt = windows::Win32::Foundation::POINT::default();
                            let _ = GetCursorPos(&mut pt);
                            show_context_menu(
                                state.hwnd_main,
                                pt.x,
                                pt.y,
                                !state.active_tab().selected_indices.is_empty(),
                            );
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CONTEXTMENU => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            if let Some(state) = state_ptr.as_ref() {
                let has_sel = !state.active_tab().selected_indices.is_empty();
                show_context_menu(hwnd, x, y, has_sel);
            }
            LRESULT(0)
        }
        WM_APP_WAKEUP => {
            if let Some(state) = state_ptr.as_mut() {
                let events = state.dispatcher.drain_batch(32);
                for event in events {
                    match event {
                        UiEvent::DirectoryPageReady(env) => {
                            let active_id = RequestIdentity::new(
                                state.session_id.clone(),
                                state.active_tab().tab_id.clone(),
                                0,
                                state.active_tab().view_epoch,
                            );
                            if !env.is_stale(&active_id)
                                && let Ok(page) = env.result
                            {
                                let before = state.active_tab().visible_item_count();
                                state.active_tab_mut().apply_page(page);
                                let count = state.active_tab().visible_item_count();
                                if count != before {
                                    let _ = SendMessageW(
                                        state.hwnd_list,
                                        LVM_SETITEMCOUNT,
                                        WPARAM(count),
                                        LPARAM(1),
                                    );
                                    state.update_status_bar();
                                }
                                let _ = InvalidateRect(state.hwnd_list, None, true);
                            }
                        }
                        UiEvent::NavigationReady(env) | UiEvent::RefreshReady(env) => {
                            let active_id = RequestIdentity::new(
                                state.session_id.clone(),
                                state.active_tab().tab_id.clone(),
                                0,
                                state.active_tab().view_epoch,
                            );
                            if !env.is_stale(&active_id) {
                                match env.result {
                                    Ok(nav) => {
                                        state.active_tab_mut().apply_navigation(nav);
                                        state.update_address_bar();
                                        state.update_tab_title();
                                        let count = state.active_tab().visible_item_count();
                                        let _ = SendMessageW(
                                            state.hwnd_list,
                                            LVM_SETITEMCOUNT,
                                            WPARAM(count),
                                            LPARAM(1),
                                        );
                                        let _ = InvalidateRect(state.hwnd_list, None, true);
                                        state.update_status_bar();
                                        state.update_sort_indicator();
                                        state.subscribe_active_folder();
                                        // Prefetch initial page
                                        state.request_page(0);
                                    }
                                    Err(err) => {
                                        // Keep the view on the last folder that actually
                                        // enumerated; otherwise the address bar and the
                                        // list disagree after a failed navigation.
                                        let applied = state.active_tab().applied_path.clone();
                                        state.active_tab_mut().current_path = applied;
                                        state.update_address_bar();
                                        // Explorer never blocks a folder switch on a modal
                                        // message box; report inline and keep the view usable.
                                        crate::diagnostics::write_line(&format!(
                                            "[navigation-error] {}",
                                            err.user_message
                                        ));
                                        state.flash_status(format!(
                                            "Cannot open folder: {}",
                                            err.user_message
                                        ));
                                    }
                                }
                            }
                        }
                        UiEvent::PreviewReady(env) => {
                            if let Ok(data) = env.result
                                && let Some(ref mut pane) = state.preview_pane
                            {
                                pane.show_preview(&data);
                            }
                        }
                        UiEvent::JobCompleted(env) => {
                            if let Ok(summary) = env.result {
                                let message = format!(
                                    "{:?} finished - {} item(s)",
                                    summary.kind, summary.total_items
                                );
                                state.flash_status(message);
                                state.trigger_refresh();
                            }
                        }
                        UiEvent::OperationError(err) => {
                            if crate::dialogs::in_modal() {
                                state.pending_error = Some(err.user_message.clone());
                            } else {
                                show_error(state.hwnd_main, "Error", &err.user_message);
                            }
                        }
                        UiEvent::WatchNotification(notif) => {
                            let active = state
                                .active_tab()
                                .current_path
                                .to_string_lossy()
                                .to_lowercase();
                            let changed = notif.dir_path.to_string_lossy().to_lowercase();
                            let relevant = notif.is_overflow
                                || changed == active
                                || changed.trim_end_matches('\\') == active.trim_end_matches('\\');
                            if relevant && !state.watch_refresh_pending {
                                state.watch_refresh_pending = true;
                                unsafe {
                                    SetTimer(hwnd, WATCH_REFRESH_TIMER, 250, None);
                                }
                            }
                        }
                        UiEvent::SearchReady(env) => {
                            if let Ok(res) = env.result {
                                state.flash_status(format!(
                                    "Found {} indexed matches for \"{}\"",
                                    res.total_matches, res.query
                                ));
                            }
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == WATCH_REFRESH_TIMER {
                unsafe {
                    let _ = KillTimer(hwnd, WATCH_REFRESH_TIMER);
                }
                if let Some(state) = state_ptr.as_mut() {
                    state.watch_refresh_pending = false;
                    state.trigger_refresh();
                }
            } else if wparam.0 == STATUS_MESSAGE_TIMER {
                unsafe {
                    let _ = KillTimer(hwnd, STATUS_MESSAGE_TIMER);
                }
                if let Some(state) = state_ptr.as_mut() {
                    state.clear_status_message();
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(state) = state_ptr.as_ref() {
                // Release native watches before the HWND disappears.
                for tab in &state.tabs {
                    let _ = explorer_app::operations::unwatch_all(&state.app_state, &tab.tab_id);
                }
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn filetime_to_systemtime(filetime: u64) -> windows::Win32::Foundation::SYSTEMTIME {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
    use windows::Win32::System::Time::FileTimeToSystemTime;

    let ft = FILETIME {
        dwLowDateTime: (filetime & 0xffff_ffff) as u32,
        dwHighDateTime: (filetime >> 32) as u32,
    };
    let mut local = FILETIME::default();
    let mut st = windows::Win32::Foundation::SYSTEMTIME::default();
    unsafe {
        if FileTimeToLocalFileTime(&ft, &mut local).is_ok() {
            let _ = FileTimeToSystemTime(&local, &mut st);
        } else {
            let _ = FileTimeToSystemTime(&ft, &mut st);
        }
    }
    st
}

/// Expands `%VAR%` sequences the way the Explorer address bar does.
fn expand_environment_variables(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '%'
            && let Some(offset) = chars[index + 1..].iter().position(|&c| c == '%')
        {
            let name: String = chars[index + 1..index + 1 + offset].iter().collect();
            if !name.is_empty()
                && let Ok(value) = std::env::var(&name)
            {
                out.push_str(&value);
                index += offset + 2;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

fn show_context_menu(hwnd: HWND, x: i32, y: i32, has_selection: bool) {
    let _modal = crate::dialogs::modal_scope();
    unsafe {
        let hmenu = CreatePopupMenu().unwrap();

        if has_selection {
            let _ = AppendMenuW(hmenu, MF_STRING, ID_ACTION_OPEN as usize, w!("Open"));
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_OPEN_EXPLORER as usize,
                w!("Open in Windows Explorer"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(hmenu, MF_STRING, ID_ACTION_CUT as usize, w!("Cut (Ctrl+X)"));
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_COPY as usize,
                w!("Copy (Ctrl+C)"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_RECYCLE as usize,
                w!("Recycle (Delete)"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_RENAME as usize,
                w!("Rename (F2)"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_ADD_FAVORITE as usize,
                w!("Add to Favorites"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_PROPERTIES as usize,
                w!("Properties (Alt+Enter)"),
            );
        } else {
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                IDC_NAV_REFRESH as usize,
                w!("Refresh (F5)"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_NEW_FOLDER as usize,
                w!("New Folder (Ctrl+Shift+N)"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_PASTE as usize,
                w!("Paste (Ctrl+V)"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_TOGGLE_HIDDEN as usize,
                w!("Toggle Hidden Files"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_TOGGLE_PREVIEW as usize,
                w!("Toggle Preview Pane (Ctrl+P)"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_ROOTS_MANAGER as usize,
                w!("Indexed Roots Manager..."),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_ACTIVITY_LOG as usize,
                w!("Activity History..."),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                ID_ACTION_PROPERTIES as usize,
                w!("Folder Properties (Alt+Enter)"),
            );
        }

        let _ = TrackPopupMenu(hmenu, TPM_RIGHTBUTTON, x, y, 0, hwnd, None);
        let _ = DestroyMenu(hmenu);
    }
}

pub fn create_main_window(
    hinstance: HINSTANCE,
    app_state: Arc<AppState>,
    runtime_handle: tokio::runtime::Handle,
    session_id: SessionId,
    initial_path: PathBuf,
) -> Result<HWND, windows::core::Error> {
    unsafe {
        let mut wc = WNDCLASSEXW::default();
        if GetClassInfoExW(hinstance, CLASS_NAME, &mut wc).is_err() {
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.lpfnWndProc = Some(wnd_proc);
            wc.hInstance = hinstance;
            wc.lpszClassName = CLASS_NAME;
            wc.hCursor = LoadCursorW(None, IDC_ARROW)?;
            wc.hbrBackground = HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _);
            RegisterClassExW(&wc);
        }

        let initial_tab = TabPresenter::new("tab-1", initial_path);

        let state = Box::new(WindowState {
            hwnd_main: HWND(std::ptr::null_mut()),
            hwnd_btn_back: HWND(std::ptr::null_mut()),
            hwnd_btn_forward: HWND(std::ptr::null_mut()),
            hwnd_btn_up: HWND(std::ptr::null_mut()),
            hwnd_btn_refresh: HWND(std::ptr::null_mut()),
            hwnd_address: HWND(std::ptr::null_mut()),
            hwnd_search: HWND(std::ptr::null_mut()),
            hwnd_btn_theme: HWND(std::ptr::null_mut()),
            hwnd_btn_preview: HWND(std::ptr::null_mut()),
            hwnd_tabs: HWND(std::ptr::null_mut()),
            hwnd_sidebar: HWND(std::ptr::null_mut()),
            hwnd_list: HWND(std::ptr::null_mut()),
            hwnd_status: HWND(std::ptr::null_mut()),
            preview_pane: None,
            sidebar_manager: None,
            app_state,
            dispatcher: Arc::new(crate::dispatcher::Dispatcher::new(
                HWND(std::ptr::null_mut()),
                128,
            )),
            runtime_handle,
            session_id,
            tabs: vec![initial_tab],
            active_tab_idx: 0,
            dpi: 96,
            font: None,
            nav_font: None,
            is_dark_mode: false,
            next_tab_id: 2,
            watch_refresh_pending: false,
            icon_cache: std::collections::HashMap::new(),
            status_message: None,
            dark_background: None,
            dark_control: None,
            pending_error: None,
        });

        let state_ptr = Box::into_raw(state);

        // Window size ~1200x800 as specified in requirement P01. Created hidden so the
        // dispatcher can be bound to the real HWND before the first navigation starts.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            w!("Rust Explorer"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1200,
            800,
            None,
            None,
            hinstance,
            Some(state_ptr as *mut _),
        )?;

        // Update real DPI
        let dpi = GetDpiForWindow(hwnd);
        let state = &mut *state_ptr;
        state.dpi = if dpi > 0 { dpi } else { 96 };
        state.dispatcher.set_hwnd(hwnd);

        state.rebuild_font();

        // Start watcher notification forwarding
        let mut notif_rx = state.app_state.watch_service.subscribe_notifications();
        let disp = state.dispatcher.clone();
        state.runtime_handle.spawn(async move {
            while let Ok(notif) = notif_rx.recv().await {
                disp.enqueue(UiEvent::WatchNotification(notif));
            }
        });

        // Layout controls, then reveal the window and load the initial folder.
        state.layout_children();
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        state.trigger_navigate(state.active_tab().current_path.clone());

        Ok(hwnd)
    }
}
