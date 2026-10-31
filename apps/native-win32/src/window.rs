#![allow(unsafe_op_in_unsafe_fn, dead_code)]

use crate::controls::*;
use crate::dialogs::{
    confirm_recycle, prompt_input, refuse_permanent_delete, show_activity_dialog, show_error,
    show_info, show_roots_manager_dialog,
};
use crate::dispatcher::{SharedDispatcher, UiEvent, WM_APP_WAKEUP};
use crate::presenter::{PAGE_SIZE, TabPresenter};
use crate::preview::PreviewPane;
use crate::resources::SafeFont;
use crate::search::{self, SearchView};
use crate::sidebar::SidebarManager;
use crate::theme::{self, DARK_PANEL, ThemeControl, ThemePreference, ThemeState};
use explorer_app::requests::{CompletionEnvelope, PageRequest, RequestIdentity};
use explorer_app::state::AppState;
use explorer_domain::ids::{ItemToken, SessionId};
use explorer_domain::models::{EntryKind, SortColumn, SortDirection};
use explorer_index::RootState;
use explorer_win::known_folders::{get_logical_drives, get_standard_known_folders};
use std::path::PathBuf;
use std::sync::Arc;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
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
    pub theme: ThemeState,
    pub theme_applying: bool,
    pub status_parts: [String; 3],
    pub search: SearchView,
    pub next_tab_id: u64,
    pub watch_refresh_pending: bool,
    pub icon_cache: std::collections::HashMap<String, i32>,
    pub status_message: Option<String>,
    /// Owned buffer for `LVM_SETEMPTYTEXT`, which stores the pointer.
    pub empty_text: Vec<u16>,
    /// Error captured while a modal dialog was open; surfaced afterwards.
    pub pending_error: Option<String>,
}

/// `LVM_SETEMPTYTEXT` (not exported by the windows crate bindings).
const LVM_SETEMPTYTEXT: u32 = 0x1000 + 204;

/// Debounce timer for filesystem watch notifications on the active folder.
const WATCH_REFRESH_TIMER: usize = 1;
/// Message lifetime for transient status-bar feedback.
const STATUS_MESSAGE_TIMER: usize = 2;
/// Debounce for the deep indexed search box.
const SEARCH_DEBOUNCE_TIMER: usize = 3;
/// Re-run the deep search while indexed roots are still being crawled.
const SEARCH_REFRESH_TIMER: usize = 4;

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
        self.exit_search(true);
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
        self.exit_search(true);
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
        if self.search.active {
            self.run_search();
            return;
        }
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

    pub fn on_search_changed(&mut self, raw: &str) {
        let query = raw.trim().to_string();
        unsafe {
            let _ = KillTimer(self.hwnd_main, SEARCH_DEBOUNCE_TIMER);
        }
        if query.is_empty() {
            self.exit_search(false);
            return;
        }

        if !self.search.active {
            self.search.active = true;
            self.search.auto_indexed_path = None;
            self.search.auto_index_pending = false;
            self.search.auto_index_attempts = 0;
            self.active_tab_mut().selected_indices.clear();
            self.clear_list_selection();
            if let Some(pane) = self.preview_pane.as_mut() {
                pane.clear();
            }
            self.apply_search_columns();
        }

        self.search.begin(query);
        self.apply_list_colors();
        self.set_list_count(0);
        self.update_status_bar();

        unsafe {
            let _ = SetTimer(
                self.hwnd_main,
                SEARCH_DEBOUNCE_TIMER,
                search::DEBOUNCE_MS,
                None,
            );
        }
    }

    pub fn run_search(&mut self) {
        unsafe {
            let _ = KillTimer(self.hwnd_main, SEARCH_DEBOUNCE_TIMER);
        }
        let query = self.search.query.trim().to_string();
        if query.is_empty() {
            self.exit_search(true);
            return;
        }

        self.search.active = true;
        self.search.query = query.clone();
        self.search.generation = self.search.generation.wrapping_add(1);
        self.search.pending = true;
        self.search.error = None;
        self.ensure_index_roots();

        let generation = self.search.generation;
        let identity = RequestIdentity::new(
            self.session_id.clone(),
            self.active_tab().tab_id.clone(),
            0,
            generation,
        );
        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();

        self.runtime_handle.spawn(async move {
            let q = query;
            let res = tokio::task::spawn_blocking(move || {
                explorer_app::operations::search_indexed(
                    &app_state,
                    &q,
                    None,
                    1,
                    search::RESULT_LIMIT,
                )
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
                identity,
                result: res,
            }));
        });

        self.update_status_bar();
    }

    fn ensure_index_roots(&mut self) {
        let roots = match explorer_app::operations::list_indexed_roots(&self.app_state) {
            Ok(roots) => roots,
            Err(error) => {
                self.search.error = Some(error.user_message.clone());
                return;
            }
        };

        self.search.indexing = roots
            .iter()
            .any(|root| matches!(root.state, RootState::Scanning | RootState::NotIndexed));
        if !roots.is_empty() {
            self.search.auto_index_pending = false;
            return;
        }

        let path = self.active_tab().current_path.clone();
        if self.search.auto_indexed_path.as_ref() == Some(&path) {
            return;
        }
        self.search.auto_indexed_path = Some(path.clone());
        self.search.auto_index_pending = true;
        self.search.auto_index_attempts = 0;

        let Some(token) = self.active_tab().folder_token.clone() else {
            self.search.auto_index_pending = false;
            self.flash_status("Folder is not ready for indexing yet");
            return;
        };

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        self.runtime_handle.spawn(async move {
            if let Err(error) = explorer_app::operations::add_indexed_root(&app_state, &token).await
            {
                dispatcher.enqueue(UiEvent::SearchIndexFailed(error));
            }
        });

        self.flash_status(format!("Indexing {} for deep search", path.display()));
        unsafe {
            let _ = SetTimer(
                self.hwnd_main,
                SEARCH_REFRESH_TIMER,
                search::REFRESH_MS,
                None,
            );
        }
    }

    fn schedule_search_refresh(&mut self) {
        let roots =
            explorer_app::operations::list_indexed_roots(&self.app_state).unwrap_or_default();
        let indexing = roots
            .iter()
            .any(|root| matches!(root.state, RootState::Scanning | RootState::NotIndexed));
        if !roots.is_empty() {
            self.search.auto_index_pending = false;
        } else if self.search.auto_index_pending {
            self.search.auto_index_attempts = self.search.auto_index_attempts.saturating_add(1);
            if self.search.auto_index_attempts >= 3 {
                self.search.auto_index_pending = false;
            }
        }
        self.search.indexing = indexing;

        let should_refresh = (indexing || self.search.auto_index_pending)
            && self.search.active
            && !self.search.query.is_empty();
        unsafe {
            if should_refresh {
                let _ = SetTimer(
                    self.hwnd_main,
                    SEARCH_REFRESH_TIMER,
                    search::REFRESH_MS,
                    None,
                );
            } else {
                let _ = KillTimer(self.hwnd_main, SEARCH_REFRESH_TIMER);
            }
        }
    }

    pub fn exit_search(&mut self, clear_query: bool) {
        unsafe {
            let _ = KillTimer(self.hwnd_main, SEARCH_DEBOUNCE_TIMER);
            let _ = KillTimer(self.hwnd_main, SEARCH_REFRESH_TIMER);
        }

        let was_active = self.search.active;
        self.search.clear();
        if clear_query {
            unsafe {
                let _ = SetWindowTextW(self.hwnd_search, w!(""));
            }
        }
        if !was_active {
            self.update_status_bar();
            return;
        }

        self.active_tab_mut().set_filter("");
        self.active_tab_mut().selected_indices.clear();
        if let Some(pane) = self.preview_pane.as_mut() {
            pane.clear();
        }
        self.apply_browse_columns();
        self.clear_list_selection();
        self.set_list_count(self.active_tab().visible_item_count());
        self.apply_list_colors();
        self.update_sort_indicator();
        self.update_status_bar();
    }

    fn set_list_count(&self, count: usize) {
        unsafe {
            let _ = SendMessageW(self.hwnd_list, LVM_SETITEMCOUNT, WPARAM(count), LPARAM(1));
            let _ = InvalidateRect(self.hwnd_list, None, true);
        }
    }

    fn clear_list_selection(&self) {
        unsafe {
            let mut item = LVITEMW {
                state: LIST_VIEW_ITEM_STATE_FLAGS(0),
                stateMask: LVIS_SELECTED,
                ..Default::default()
            };
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETITEMSTATE,
                WPARAM(usize::MAX),
                LPARAM(&mut item as *mut _ as isize),
            );
        }
    }

    fn set_column_label(&self, index: usize, label: &str) {
        let mut text: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
        let mut column = LVCOLUMNW {
            mask: LVCF_TEXT,
            pszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        unsafe {
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETCOLUMNW,
                WPARAM(index),
                LPARAM(&mut column as *mut _ as isize),
            );
        }
    }

    fn apply_search_columns(&self) {
        self.set_column_label(0, "Name");
        self.set_column_label(1, "Location");
        self.set_column_label(2, "Size");
        self.set_column_label(3, "Date modified");
        self.layout_children();
        self.update_sort_indicator();
    }

    fn apply_list_colors(&self) {
        let (background, text) = if self.theme.dark && !self.theme.high_contrast {
            (DARK_PANEL, theme::DARK_TEXT)
        } else {
            unsafe { (GetSysColor(COLOR_WINDOW), GetSysColor(COLOR_WINDOWTEXT)) }
        };
        unsafe {
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETBKCOLOR,
                WPARAM(0),
                LPARAM(background as isize),
            );
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETTEXTCOLOR,
                WPARAM(0),
                LPARAM(text as isize),
            );
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETTEXTBKCOLOR,
                WPARAM(0),
                LPARAM(CLR_NONE as isize),
            );
            let _ = InvalidateRect(self.hwnd_list, None, true);
        }
    }

    fn apply_browse_columns(&self) {
        self.set_column_label(0, "Name");
        self.set_column_label(1, "Type");
        self.set_column_label(2, "Size");
        self.set_column_label(3, "Date modified");
        self.layout_children();
    }

    fn search_selected_count(&self) -> usize {
        unsafe {
            SendMessageW(self.hwnd_list, LVM_GETSELECTEDCOUNT, WPARAM(0), LPARAM(0)).0 as usize
        }
    }

    pub fn trigger_preview_load(&mut self, item_idx: usize) {
        if self.search.active {
            return;
        }
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
        self.exit_search(true);
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
        self.exit_search(true);
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
            self.exit_search(true);
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
        if self.search.active {
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
            return;
        }

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

    fn reject_search_action(&mut self) -> bool {
        if self.search.active {
            self.flash_status("Not available in search results");
            true
        } else {
            false
        }
    }

    pub fn action_new_folder(&mut self) {
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
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
        if self.search.active {
            self.open_selected_search_result();
            return;
        }
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

    fn selected_search_row(&self) -> Option<usize> {
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
            None
        } else {
            Some(selected as usize)
        }
    }

    fn selected_search_item(&self) -> Option<explorer_index::SearchResultItem> {
        let row = self.selected_search_row()?;
        self.search.results.get(row).cloned()
    }

    fn open_selected_search_result(&mut self) {
        let Some(item) = self.selected_search_item() else {
            return;
        };
        if item.kind != "file" {
            match explorer_app::operations::path_from_utf16(&item.path_utf16, "search_open") {
                Ok(path) => self.trigger_navigate(path),
                Err(error) => self.flash_status(error.user_message.clone()),
            }
            return;
        }

        let app_state = self.app_state.clone();
        let dispatcher = self.dispatcher.clone();
        let path_utf16 = item.path_utf16;
        self.runtime_handle.spawn(async move {
            if let Err(error) =
                explorer_app::operations::open_path_with(&app_state, &path_utf16, true).await
            {
                dispatcher.enqueue(UiEvent::OperationError(error));
            }
        });
    }

    fn open_selected_search_containing_folder(&mut self) {
        let Some(item) = self.selected_search_item() else {
            return;
        };
        match explorer_app::operations::path_from_utf16(&item.path_utf16, "search_containing") {
            Ok(path) => match path.parent() {
                Some(parent) => self.trigger_navigate(parent.to_path_buf()),
                None => self.flash_status("This item has no containing folder"),
            },
            Err(error) => self.flash_status(error.user_message.clone()),
        }
    }

    fn copy_selected_search_path(&mut self) {
        let Some(item) = self.selected_search_item() else {
            return;
        };
        let path = match explorer_app::operations::path_from_utf16(&item.path_utf16, "search_copy")
        {
            Ok(path) => explorer_win::path::strip_extended_prefix(&path),
            Err(error) => {
                self.flash_status(error.user_message.clone());
                return;
            }
        };
        let path_utf16 = explorer_win::path::path_to_wide(&path);
        let owner = self.hwnd_main.0 as usize;
        let dispatcher = self.dispatcher.clone();
        self.runtime_handle.spawn(async move {
            match explorer_app::operations::clipboard_write_path(&path_utf16, owner).await {
                Ok(()) => dispatcher.enqueue(UiEvent::StatusMessage("Path copied".to_string())),
                Err(error) => dispatcher.enqueue(UiEvent::OperationError(error)),
            }
        });
    }

    pub fn search_icon_index(&mut self, row: usize) -> i32 {
        let Some(item) = self.search.results.get(row) else {
            return 0;
        };
        let (key, icon_key, is_dir) = match item.kind.as_str() {
            "directory" => ("search:dir".to_string(), "", true),
            "reparse_point" => ("search:link".to_string(), "link", false),
            _ => {
                if item.extension.is_empty() {
                    ("search:file".to_string(), "file", false)
                } else {
                    (
                        format!("search:{}", item.extension.to_ascii_lowercase()),
                        item.extension.as_str(),
                        false,
                    )
                }
            }
        };
        if let Some(index) = self.icon_cache.get(&key) {
            return *index;
        }
        let index = crate::icons::icon_index(icon_key, is_dir);
        self.icon_cache.insert(key, index);
        index
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
        if self.search.active {
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
                    item.mask = HDI_FORMAT;
                    item.fmt =
                        HEADER_CONTROL_FORMAT_FLAGS(item.fmt.0 & !(HDF_SORTUP.0 | HDF_SORTDOWN.0));
                    let _ = SendMessageW(
                        header,
                        HDM_SETITEMW,
                        WPARAM(current),
                        LPARAM(&mut item as *mut _ as isize),
                    );
                }
            }
            return;
        }

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
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
            return;
        }
        let next = !self.active_tab().show_hidden;
        self.active_tab_mut().show_hidden = next;
        self.active_tab_mut().set_filter("");
        // Hidden/system entries are excluded by the snapshot, so re-enumerate.
        self.trigger_refresh();
    }

    pub fn action_cycle_theme(&mut self) {
        self.theme.preference = self.theme.preference.cycle();
        self.theme.refresh();
        self.apply_theme();
        if self.persist_theme_preference() {
            self.flash_status(format!("Theme: {}", self.theme.preference.label()));
        }
    }

    pub fn update_theme_button(&self) {
        let label = format!("Theme: {}", self.theme.preference.label());
        let mut utf16: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let _ = SetWindowTextW(self.hwnd_btn_theme, PCWSTR(utf16.as_mut_ptr()));
        }
    }

    fn persist_theme_preference(&mut self) -> bool {
        match explorer_app::operations::load_settings(&self.app_state) {
            Ok(mut settings) => {
                settings.theme = self.theme.preference.as_setting().to_string();
                if let Err(error) =
                    explorer_app::operations::save_settings(&self.app_state, &settings)
                {
                    self.flash_status(format!("Theme not saved: {}", error));
                    false
                } else {
                    true
                }
            }
            Err(error) => {
                self.flash_status(format!("Theme not saved: {}", error));
                false
            }
        }
    }

    /// Applies the resolved light/dark palette across every shell surface.
    pub fn apply_theme(&mut self) {
        if self.theme_applying {
            return;
        }
        self.theme_applying = true;
        let dark = self.theme.dark;
        let high_contrast = self.theme.high_contrast;

        theme::set_active(dark, high_contrast);
        if !high_contrast {
            theme::set_preferred_app_mode(self.theme.app_mode());
            theme::flush_menu_themes();
            theme::apply_window_chrome(self.hwnd_main, dark);
            theme::allow_dark_mode_for_window(self.hwnd_main, dark);
            self.apply_control_themes(dark);
        } else {
            theme::apply_window_chrome(self.hwnd_main, false);
            theme::allow_dark_mode_for_window(self.hwnd_main, false);
            self.apply_control_themes(false);
        }

        unsafe {
            let (gridline_style, status_color, tree_background, tree_text) =
                if dark && !high_contrast {
                    (
                        0,
                        DARK_PANEL as isize,
                        DARK_PANEL as isize,
                        theme::DARK_TEXT as isize,
                    )
                } else {
                    (
                        LVS_EX_GRIDLINES as isize,
                        CLR_DEFAULT as isize,
                        GetSysColor(COLOR_WINDOW) as isize,
                        GetSysColor(COLOR_WINDOWTEXT) as isize,
                    )
                };
            let _ = SendMessageW(
                self.hwnd_list,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                WPARAM(LVS_EX_GRIDLINES as usize),
                LPARAM(gridline_style),
            );
            let _ = SendMessageW(
                self.hwnd_status,
                SB_SETBKCOLOR,
                WPARAM(0),
                LPARAM(status_color),
            );
            let _ = SendMessageW(
                self.hwnd_sidebar,
                TVM_SETBKCOLOR,
                WPARAM(0),
                LPARAM(tree_background),
            );
            let _ = SendMessageW(
                self.hwnd_sidebar,
                TVM_SETTEXTCOLOR,
                WPARAM(0),
                LPARAM(tree_text),
            );
            let _ = RedrawWindow(
                self.hwnd_main,
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_FRAME,
            );
        }

        self.update_status_bar();
        self.update_theme_button();
        self.apply_list_colors();
        self.theme_applying = false;
    }

    fn apply_control_themes(&self, dark: bool) {
        for (control, kind) in [
            (self.hwnd_btn_back, ThemeControl::Button),
            (self.hwnd_btn_forward, ThemeControl::Button),
            (self.hwnd_btn_up, ThemeControl::Button),
            (self.hwnd_btn_refresh, ThemeControl::Button),
            (self.hwnd_address, ThemeControl::Edit),
            (self.hwnd_search, ThemeControl::Edit),
            (self.hwnd_btn_theme, ThemeControl::Button),
            (self.hwnd_btn_preview, ThemeControl::Button),
            (self.hwnd_tabs, ThemeControl::Tab),
            (self.hwnd_sidebar, ThemeControl::Tree),
            (self.hwnd_list, ThemeControl::List),
            (self.hwnd_status, ThemeControl::Status),
        ] {
            theme::apply_control_theme(control, dark, kind);
        }

        let header = unsafe { SendMessageW(self.hwnd_list, LVM_GETHEADER, WPARAM(0), LPARAM(0)) };
        if header.0 != 0 {
            theme::apply_control_theme(HWND(header.0 as *mut _), dark, ThemeControl::Header);
        }
        if let Some(pane) = self.preview_pane.as_ref() {
            pane.apply_theme(dark);
        }
    }

    pub fn action_add_favorite(&mut self) {
        if self.reject_search_action() {
            return;
        }
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
        if self.reject_search_action() {
            return;
        }
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

    pub fn update_status_bar(&mut self) {
        let (part1, part2, part3) = if self.search.active {
            let (part1, part2) = self.search.status_parts();
            let selected = self.search_selected_count();
            let part3 = if let Some(message) = self.status_message.clone() {
                message
            } else if selected > 0 {
                format!(
                    "{} item{} selected",
                    selected,
                    if selected == 1 { "" } else { "s" }
                )
            } else {
                String::new()
            };
            (part1, part2, part3)
        } else {
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
            (part1, part2, part3)
        };

        self.status_parts = [part1, part2, part3];
        let dark = self.theme.dark;

        unsafe {
            for (index, text) in self.status_parts.iter().enumerate() {
                if dark {
                    let _ = SendMessageW(
                        self.hwnd_status,
                        SB_SETTEXTW,
                        WPARAM(index | SBT_OWNERDRAW as usize),
                        LPARAM(index as isize),
                    );
                } else {
                    let utf16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
                    let _ = SendMessageW(
                        self.hwnd_status,
                        SB_SETTEXTW,
                        WPARAM(index),
                        LPARAM(utf16.as_ptr() as isize),
                    );
                }
            }
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
        let theme_btn_w = scale(120);
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
            let item_width = scale(180);
            let item_size = ((tabs_h as u32) << 16) | (item_width as u32 & 0xffff);
            let _ = SendMessageW(
                self.hwnd_tabs,
                TCM_SETITEMSIZE,
                WPARAM(0),
                LPARAM(item_size as isize),
            );
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

        // Stretch the flexible column so the report fills the view, leaving room for
        // the vertical scrollbar the control reserves.
        let size_w = scale(96);
        let date_w = scale(160);
        let scrollbar_w = unsafe { GetSystemMetrics(SM_CXVSCROLL) };
        let (name_w, middle_w) = if self.search.active {
            let name_w = scale(240);
            let middle_w =
                (list_w - name_w - size_w - date_w - scale(4) - scrollbar_w).max(scale(180));
            (name_w, middle_w)
        } else {
            let type_w = scale(96);
            let name_w =
                (list_w - type_w - size_w - date_w - scale(4) - scrollbar_w).max(scale(180));
            (name_w, type_w)
        };
        for (column, column_width) in [name_w, middle_w, size_w, date_w].into_iter().enumerate() {
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
            // Enter: run the deep indexed search immediately.
            let state_ptr = refdata as *mut WindowState;
            if let Some(state) = state_ptr.as_mut() {
                let len = GetWindowTextLengthW(hwnd) as usize;
                let mut buf = vec![0u16; len + 1];
                let copied = GetWindowTextW(hwnd, &mut buf) as usize;
                let query = String::from_utf16_lossy(&buf[..copied]);
                state.on_search_changed(query.trim());
                if state.search.active {
                    state.run_search();
                }
                return LRESULT(0);
            }
        } else if wparam.0 == 0x1B {
            // Escape: leave search and restore the browsed folder.
            let state_ptr = refdata as *mut WindowState;
            if let Some(state) = state_ptr.as_mut() {
                state.exit_search(true);
                return LRESULT(0);
            }
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn preview_container_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    refdata: usize,
) -> LRESULT {
    let state_ptr = refdata as *mut WindowState;
    if let Some(state) = state_ptr.as_ref()
        && state.theme.dark
    {
        let hdc = HDC(wparam.0 as *mut _);
        match msg {
            WM_ERASEBKGND => {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(hdc, &rect, state.theme.panel_brush());
                return LRESULT(1);
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
                let control = HWND(lparam.0 as *mut _);
                let is_text = state
                    .preview_pane
                    .as_ref()
                    .is_some_and(|pane| pane.hwnd_text == control);
                let (text, background, brush) = if is_text {
                    (
                        theme::DARK_TEXT,
                        theme::DARK_INPUT,
                        state.theme.input_brush(),
                    )
                } else {
                    (
                        theme::DARK_TEXT,
                        theme::DARK_PANEL,
                        state.theme.panel_brush(),
                    )
                };
                let _ = SetTextColor(hdc, COLORREF(text));
                let _ = SetBkColor(hdc, COLORREF(background));
                let _ = SetBkMode(hdc, TRANSPARENT);
                return LRESULT(brush.0 as isize);
            }
            _ => {}
        }
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn status_bar_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    refdata: usize,
) -> LRESULT {
    let state_ptr = refdata as *mut WindowState;
    if msg == WM_ERASEBKGND
        && let Some(state) = state_ptr.as_ref()
        && state.theme.dark
    {
        let hdc = HDC(wparam.0 as *mut _);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let _ = FillRect(hdc, &rect, state.theme.panel_brush());
        return LRESULT(1);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn tab_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    refdata: usize,
) -> LRESULT {
    let state_ptr = refdata as *mut WindowState;
    if msg == WM_ERASEBKGND
        && let Some(state) = state_ptr.as_ref()
        && state.theme.dark
    {
        let hdc = HDC(wparam.0 as *mut _);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let _ = FillRect(hdc, &rect, state.theme.background_brush());
        return LRESULT(1);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn list_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    refdata: usize,
) -> LRESULT {
    let state_ptr = refdata as *mut WindowState;
    if msg == WM_PAINT
        && let Some(state) = state_ptr.as_ref()
        && state.theme.dark
        && !state.theme.high_contrast
    {
        let empty = if state.search.active {
            state.search.results.is_empty()
        } else {
            state.active_tab().visible_item_count() == 0
        };
        if empty {
            let message = if state.search.active {
                if state.search.pending {
                    Some("Searching indexed roots...".to_string())
                } else if let Some(error) = &state.search.error {
                    Some(error.clone())
                } else {
                    Some("No indexed matches.".to_string())
                }
            } else if state.active_tab().total_items == 0 {
                Some(String::from_utf16_lossy(
                    &state.empty_text[..state.empty_text.len().saturating_sub(1)],
                ))
            } else {
                Some(String::new())
            };

            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let _ = FillRect(hdc, &rect, state.theme.panel_brush());
            if let Some(message) = message
                && !message.is_empty()
            {
                let mut text: Vec<u16> = message.encode_utf16().collect();
                let previous_font = state
                    .font
                    .as_ref()
                    .map(|font| SelectObject(hdc, HGDIOBJ(font.handle().0)));
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = SetTextColor(hdc, COLORREF(theme::DARK_TEXT_SECONDARY));
                let _ = DrawTextW(
                    hdc,
                    &mut text,
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
                if let Some(previous) = previous_font {
                    let _ = SelectObject(hdc, previous);
                }
            }
            let _ = EndPaint(hwnd, &ps);
            return LRESULT(0);
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
            if !state.theme.high_contrast {
                theme::allow_dark_mode_for_window(hwnd, state.theme.dark);
            }

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
                LPARAM(w!("Search all indexed roots (Ctrl+F)").0 as isize),
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
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WINDOW_STYLE(TCS_OWNERDRAWFIXED),
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

            let _ = SetWindowSubclass(
                state.hwnd_tabs,
                Some(tab_subclass_proc),
                5,
                state_raw as usize,
            );

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

            let _ = SetWindowSubclass(
                state.hwnd_list,
                Some(list_subclass_proc),
                6,
                state_raw as usize,
            );

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
            let _ = SendMessageW(
                state.hwnd_list,
                LVM_SETEMPTYTEXT,
                WPARAM(0),
                LPARAM(state.empty_text.as_ptr() as isize),
            );

            // 8. Preview Pane
            let pane = PreviewPane::new(hwnd, hinst);
            let _ = SetWindowSubclass(
                pane.hwnd_container,
                Some(preview_container_subclass_proc),
                3,
                state_raw as usize,
            );
            state.preview_pane = Some(pane);

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

            let _ = SetWindowSubclass(
                state.hwnd_status,
                Some(status_bar_subclass_proc),
                4,
                state_raw as usize,
            );

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
                && state.theme.dark
            {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(
                    HDC(wparam.0 as *mut _),
                    &rect,
                    state.theme.background_brush(),
                );
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            if let Some(state) = state_ptr.as_ref()
                && state.theme.dark
            {
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetTextColor(hdc, COLORREF(theme::DARK_TEXT));
                let _ = SetBkColor(hdc, COLORREF(theme::DARK_INPUT));
                return LRESULT(state.theme.input_brush().0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            if let Some(state) = state_ptr.as_ref()
                && state.theme.dark
            {
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetTextColor(hdc, COLORREF(theme::DARK_TEXT));
                let _ = SetBkColor(hdc, COLORREF(theme::DARK_PANEL));
                let _ = SetBkMode(hdc, TRANSPARENT);
                return LRESULT(state.theme.panel_brush().0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DRAWITEM => {
            if let Some(state) = state_ptr.as_ref() {
                let draw = &*(lparam.0 as *const DRAWITEMSTRUCT);
                if draw.hwndItem == state.hwnd_status && state.theme.dark {
                    let part = draw.itemID as usize;
                    let text = state
                        .status_parts
                        .get(part)
                        .map(String::as_str)
                        .unwrap_or("");
                    unsafe {
                        let _ = FillRect(draw.hDC, &draw.rcItem, state.theme.panel_brush());
                        let _ = SetBkMode(draw.hDC, TRANSPARENT);
                        let _ = SetTextColor(
                            draw.hDC,
                            COLORREF(if part == 0 {
                                theme::DARK_TEXT
                            } else {
                                theme::DARK_TEXT_SECONDARY
                            }),
                        );
                        let previous_font = state
                            .font
                            .as_ref()
                            .map(|font| SelectObject(draw.hDC, HGDIOBJ(font.handle().0)));
                        let mut wide: Vec<u16> = text.encode_utf16().collect();
                        let mut rect = draw.rcItem;
                        let pad = state.scale(6);
                        rect.left += pad;
                        rect.right -= pad;
                        if !wide.is_empty() {
                            let _ = DrawTextW(
                                draw.hDC,
                                &mut wide,
                                &mut rect,
                                DT_SINGLELINE
                                    | DT_VCENTER
                                    | DT_LEFT
                                    | DT_END_ELLIPSIS
                                    | DT_NOPREFIX,
                            );
                        }
                        if let Some(previous) = previous_font {
                            let _ = SelectObject(draw.hDC, previous);
                        }
                    }
                    return LRESULT(1);
                }

                if draw.hwndItem == state.hwnd_tabs {
                    let selected = unsafe {
                        SendMessageW(state.hwnd_tabs, TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0
                    } as usize;
                    let is_selected = draw.itemID as usize == selected;
                    let dark = state.theme.dark && !state.theme.high_contrast;
                    let mut text = [0u16; 256];
                    let mut item = TCITEMW {
                        mask: TCIF_TEXT,
                        pszText: PWSTR(text.as_mut_ptr()),
                        cchTextMax: text.len() as i32,
                        ..Default::default()
                    };
                    unsafe {
                        let _ = SendMessageW(
                            state.hwnd_tabs,
                            TCM_GETITEMW,
                            WPARAM(draw.itemID as usize),
                            LPARAM(&mut item as *mut _ as isize),
                        );
                        let length = text.iter().position(|&c| c == 0).unwrap_or(text.len());
                        let text_str = String::from_utf16_lossy(&text[..length]);
                        let (brush, text_color, accent_brush) = if dark {
                            (
                                if is_selected {
                                    state.theme.panel_brush()
                                } else {
                                    state.theme.background_brush()
                                },
                                COLORREF(if is_selected {
                                    theme::DARK_TEXT
                                } else {
                                    theme::DARK_TEXT_SECONDARY
                                }),
                                state.theme.accent_brush(),
                            )
                        } else {
                            (
                                GetSysColorBrush(if is_selected {
                                    COLOR_WINDOW
                                } else {
                                    COLOR_BTNFACE
                                }),
                                COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                                GetSysColorBrush(COLOR_HIGHLIGHT),
                            )
                        };
                        let _ = FillRect(draw.hDC, &draw.rcItem, brush);
                        if is_selected && dark {
                            let mut accent_rect = draw.rcItem;
                            accent_rect.top = accent_rect.bottom - state.scale(2);
                            accent_rect.left += state.scale(4);
                            accent_rect.right -= state.scale(4);
                            let _ = FillRect(draw.hDC, &accent_rect, accent_brush);
                        }
                        let _ = SetBkMode(draw.hDC, TRANSPARENT);
                        let _ = SetTextColor(draw.hDC, text_color);
                        let previous_font = state
                            .font
                            .as_ref()
                            .map(|font| SelectObject(draw.hDC, HGDIOBJ(font.handle().0)));
                        let mut wide: Vec<u16> = text_str.encode_utf16().collect();
                        if !wide.is_empty() {
                            let mut rect = draw.rcItem;
                            rect.left += state.scale(6);
                            rect.right -= state.scale(6);
                            rect.top += state.scale(2);
                            let _ = DrawTextW(
                                draw.hDC,
                                &mut wide,
                                &mut rect,
                                DT_CENTER
                                    | DT_VCENTER
                                    | DT_SINGLELINE
                                    | DT_END_ELLIPSIS
                                    | DT_NOPREFIX,
                            );
                        }
                        if let Some(previous) = previous_font {
                            let _ = SelectObject(draw.hDC, previous);
                        }
                    }
                    return LRESULT(1);
                }
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
                // Deep indexed search on every text change.
                if id == IDC_EDIT_SEARCH && code as u32 == EN_CHANGE {
                    let len = GetWindowTextLengthW(state.hwnd_search) as usize;
                    let mut buf = vec![0u16; len + 1];
                    let copied = GetWindowTextW(state.hwnd_search, &mut buf) as usize;
                    let query = String::from_utf16_lossy(&buf[..copied]);
                    state.on_search_changed(&query);
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
                        state.action_cycle_theme();
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
                    ID_ACTION_SEARCH_OPEN_CONTAINING => {
                        state.open_selected_search_containing_folder();
                    }
                    ID_ACTION_SEARCH_COPY_PATH => {
                        state.copy_selected_search_path();
                    }
                    ID_ACTION_SEARCH_CLEAR => {
                        state.exit_search(true);
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
                                plvdi.item.iImage = if state.search.active {
                                    state.search_icon_index(row_idx)
                                } else {
                                    state.icon_index_for(row_idx)
                                };
                            }

                            if (plvdi.item.mask.0 & LVIF_TEXT.0) != 0 {
                                if state.search.active {
                                    let text = state
                                        .search
                                        .results
                                        .get(row_idx)
                                        .map(|item| match sub_idx {
                                            0 => item.display_name.clone(),
                                            1 => explorer_win::path::to_display_string(
                                                std::path::Path::new(&item.path),
                                            ),
                                            2 => format_size(item.size_bytes),
                                            3 => format_modified(item.modified_filetime),
                                            _ => String::new(),
                                        })
                                        .unwrap_or_default();
                                    fill_listview_text(plvdi, &text);
                                } else if let Some(entry) =
                                    state.active_tab().get_visible_entry(row_idx)
                                {
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
                                        2 => format_size(entry.size_bytes),
                                        3 => format_modified(entry.modified_filetime),
                                        _ => String::new(),
                                    };
                                    fill_listview_text(plvdi, &text);
                                } else {
                                    // Missing page: schedule load
                                    let actual_idx = state.active_tab().get_actual_index(row_idx);
                                    let offset = (actual_idx / PAGE_SIZE) * PAGE_SIZE;
                                    state.request_page(offset);
                                    fill_listview_text(plvdi, "...");
                                }
                            }
                            return LRESULT(0);
                        }
                        LVN_ODCACHEHINT => {
                            if state.search.active {
                                return LRESULT(0);
                            }
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
                            if state.search.active {
                                return LRESULT(0);
                            }
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
                                if state.search.active {
                                    state.update_status_bar();
                                    return LRESULT(0);
                                }
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
                            if state.search.active {
                                show_search_context_menu(
                                    state.hwnd_main,
                                    pt.x,
                                    pt.y,
                                    state.search_selected_count() > 0,
                                );
                            } else {
                                show_context_menu(
                                    state.hwnd_main,
                                    pt.x,
                                    pt.y,
                                    !state.active_tab().selected_indices.is_empty(),
                                );
                            }
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
                if state.search.active {
                    show_search_context_menu(hwnd, x, y, state.search_selected_count() > 0);
                } else {
                    let has_sel = !state.active_tab().selected_indices.is_empty();
                    show_context_menu(hwnd, x, y, has_sel);
                }
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
                                if !state.search.active {
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
                                        if !state.search.active {
                                            let count = state.active_tab().visible_item_count();
                                            let _ = SendMessageW(
                                                state.hwnd_list,
                                                LVM_SETITEMCOUNT,
                                                WPARAM(count),
                                                LPARAM(1),
                                            );
                                            let _ = InvalidateRect(state.hwnd_list, None, true);
                                            state.update_status_bar();
                                        }
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
                            if !state.search.active
                                && let Ok(data) = env.result
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
                        UiEvent::StatusMessage(message) => {
                            state.flash_status(message);
                        }
                        UiEvent::SearchIndexFailed(error) => {
                            state.search.auto_index_pending = false;
                            state.search.auto_index_attempts = 3;
                            state.search.error = Some(error.user_message.clone());
                            state.flash_status(format!(
                                "Deep search indexing failed: {}",
                                error.user_message
                            ));
                            state.update_status_bar();
                        }
                        UiEvent::OperationError(err) => {
                            if crate::dialogs::in_modal() {
                                state.pending_error = Some(err.user_message.clone());
                            } else {
                                show_error(state.hwnd_main, "Error", &err.user_message);
                            }
                        }
                        UiEvent::WatchNotification(notif) => {
                            if state.search.active {
                                continue;
                            }
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
                            let current = RequestIdentity::new(
                                state.session_id.clone(),
                                state.active_tab().tab_id.clone(),
                                0,
                                state.search.generation,
                            );
                            if state.search.active && !env.is_stale(&current) {
                                match env.result {
                                    Ok(res) => {
                                        let count = res.results.len();
                                        state.search.apply(res);
                                        state.set_list_count(count);
                                        state.apply_list_colors();
                                        state.schedule_search_refresh();
                                        state.update_status_bar();
                                    }
                                    Err(err) => {
                                        state.search.fail(err.user_message.clone());
                                        state.set_list_count(0);
                                        state.update_status_bar();
                                    }
                                }
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
            } else if wparam.0 == SEARCH_DEBOUNCE_TIMER {
                unsafe {
                    let _ = KillTimer(hwnd, SEARCH_DEBOUNCE_TIMER);
                }
                if let Some(state) = state_ptr.as_mut()
                    && state.search.active
                    && !state.search.query.is_empty()
                {
                    state.run_search();
                }
            } else if wparam.0 == SEARCH_REFRESH_TIMER {
                unsafe {
                    let _ = KillTimer(hwnd, SEARCH_REFRESH_TIMER);
                }
                if let Some(state) = state_ptr.as_mut()
                    && state.search.active
                    && !state.search.pending
                    && !state.search.query.is_empty()
                {
                    state.run_search();
                }
            }
            LRESULT(0)
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
            if let Some(state) = state_ptr.as_mut()
                && !state.theme_applying
            {
                let changed = state.theme.refresh();
                if changed || state.theme.preference == ThemePreference::System {
                    state.apply_theme();
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
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

fn format_size(bytes: Option<u64>) -> String {
    match bytes {
        Some(bytes) if bytes >= 1024 * 1024 => {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        }
        Some(bytes) if bytes >= 1024 => format!("{:.1} KB", bytes as f64 / 1024.0),
        Some(bytes) => format!("{} B", bytes),
        None => String::new(),
    }
}

fn format_modified(filetime: Option<u64>) -> String {
    let Some(filetime) = filetime else {
        return String::new();
    };
    let st = filetime_to_systemtime(filetime);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute
    )
}

unsafe fn fill_listview_text(plvdi: &mut NMLVDISPINFOW, text: &str) {
    let text_utf16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let max_len = (plvdi.item.cchTextMax as usize).min(text_utf16.len());
    if !plvdi.item.pszText.is_null() && max_len > 0 {
        std::ptr::copy_nonoverlapping(text_utf16.as_ptr(), plvdi.item.pszText.0, max_len);
        *plvdi.item.pszText.0.add(max_len - 1) = 0;
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

fn show_search_context_menu(hwnd: HWND, x: i32, y: i32, has_selection: bool) {
    let _modal = crate::dialogs::modal_scope();
    unsafe {
        let hmenu = CreatePopupMenu().unwrap();
        let flag = |enabled: bool| {
            if enabled {
                MF_STRING
            } else {
                MF_STRING | MF_GRAYED
            }
        };

        let _ = AppendMenuW(
            hmenu,
            flag(has_selection),
            ID_ACTION_OPEN as usize,
            w!("Open"),
        );
        let _ = AppendMenuW(
            hmenu,
            flag(has_selection),
            ID_ACTION_SEARCH_OPEN_CONTAINING as usize,
            w!("Open containing folder"),
        );
        let _ = AppendMenuW(
            hmenu,
            flag(has_selection),
            ID_ACTION_SEARCH_COPY_PATH as usize,
            w!("Copy path"),
        );
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_ACTION_SEARCH_CLEAR as usize,
            w!("Clear search"),
        );
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            ID_ACTION_ROOTS_MANAGER as usize,
            w!("Indexed Roots Manager..."),
        );

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
    let settings = explorer_app::operations::load_settings(&app_state).unwrap_or_default();
    let theme = ThemeState::new(ThemePreference::from_setting(&settings.theme));

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
            theme,
            theme_applying: false,
            status_parts: Default::default(),
            search: SearchView::default(),
            next_tab_id: 2,
            watch_refresh_pending: false,
            icon_cache: std::collections::HashMap::new(),
            status_message: None,
            empty_text: "This folder is empty."
                .encode_utf16()
                .chain(Some(0))
                .collect(),
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
        state.layout_children();
        state.apply_theme();

        // Start watcher notification forwarding
        let mut notif_rx = state.app_state.watch_service.subscribe_notifications();
        let disp = state.dispatcher.clone();
        state.runtime_handle.spawn(async move {
            while let Ok(notif) = notif_rx.recv().await {
                disp.enqueue(UiEvent::WatchNotification(notif));
            }
        });

        // Reveal the window and load the initial folder.
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        state.trigger_navigate(state.active_tab().current_path.clone());

        Ok(hwnd)
    }
}
