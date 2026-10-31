#![allow(dead_code, unsafe_op_in_unsafe_fn)]

use crate::theme::{self, DialogPalette};
use explorer_app::state::AppState;
use std::cell::Cell;
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    FillRect, HDC, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Controls::EM_SETSEL;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

thread_local! {
    /// Depth of modal UI (message boxes, prompt dialogs, popup menus).
    static MODAL_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// True while a modal dialog or popup menu owns a nested message loop.
pub fn in_modal() -> bool {
    MODAL_DEPTH.with(|depth| depth.get() > 0)
}

/// RAII guard marking a nested modal message loop.
pub struct ModalGuard;

impl Drop for ModalGuard {
    fn drop(&mut self) {
        MODAL_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Marks the current scope as a nested modal loop.
///
/// While a modal dialog pumps messages, the main window procedure must not
/// mutate application state again: re-entering the dispatcher/virtual-list
/// handlers with a second mutable view of `WindowState` corrupts state.
pub fn modal_scope() -> ModalGuard {
    MODAL_DEPTH.with(|depth| depth.set(depth.get() + 1));
    ModalGuard
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageButtons {
    Ok,
    YesNo,
    YesNoCancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageResult {
    Ok,
    Yes,
    No,
    Cancel,
}

fn cancel_result(buttons: MessageButtons) -> MessageResult {
    match buttons {
        MessageButtons::Ok => MessageResult::Ok,
        MessageButtons::YesNo => MessageResult::No,
        MessageButtons::YesNoCancel => MessageResult::Cancel,
    }
}

fn show_message_box(
    hwnd: HWND,
    title: &str,
    message: &str,
    buttons: MessageButtons,
    light_flags: MESSAGEBOX_STYLE,
) -> MessageResult {
    let _guard = modal_scope();
    if theme::active_dark() {
        return show_dark_message(hwnd, title, message, buttons);
    }

    let title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    let message_utf16: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(message_utf16.as_ptr()),
            PCWSTR(title_utf16.as_ptr()),
            light_flags,
        )
    };
    if result == IDOK {
        MessageResult::Ok
    } else if result == IDYES {
        MessageResult::Yes
    } else if result == IDNO {
        MessageResult::No
    } else {
        MessageResult::Cancel
    }
}

struct MessageDialogState {
    hwnd_text: HWND,
    palette: DialogPalette,
    buttons: MessageButtons,
    result: Option<MessageResult>,
    completed: bool,
}

fn show_dark_message(
    hwnd_parent: HWND,
    title: &str,
    message: &str,
    buttons: MessageButtons,
) -> MessageResult {
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};

    const MESSAGE_CLASS: PCWSTR = w!("RustExplorerMessageDialog");

    unsafe extern "system" fn message_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MessageDialogState;
        let Some(state) = state_ptr.as_mut() else {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        };
        match msg {
            WM_ERASEBKGND => {
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(
                    HDC(wparam.0 as *mut _),
                    &rect,
                    state.palette.background_brush(),
                );
                LRESULT(1)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetTextColor(hdc, state.palette.text_color());
                let _ = SetBkColor(hdc, state.palette.input_color());
                let _ = SetBkMode(hdc, TRANSPARENT);
                LRESULT(state.palette.input_brush().0 as isize)
            }
            WM_CTLCOLORBTN => {
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetTextColor(hdc, state.palette.text_color());
                let _ = SetBkColor(hdc, state.palette.background_color());
                let _ = SetBkMode(hdc, TRANSPARENT);
                LRESULT(state.palette.background_brush().0 as isize)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as i32;
                state.result = if id == IDOK.0 {
                    Some(MessageResult::Ok)
                } else if id == IDYES.0 {
                    Some(MessageResult::Yes)
                } else if id == IDNO.0 {
                    Some(MessageResult::No)
                } else if id == IDCANCEL.0 {
                    Some(MessageResult::Cancel)
                } else {
                    None
                };
                if state.result.is_some() {
                    state.completed = true;
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                state.result = Some(cancel_result(state.buttons));
                state.completed = true;
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap().0);

        let mut wc = WNDCLASSEXW::default();
        if GetClassInfoExW(hinstance, MESSAGE_CLASS, &mut wc).is_err() {
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.lpfnWndProc = Some(message_wnd_proc);
            wc.hInstance = hinstance;
            wc.lpszClassName = MESSAGE_CLASS;
            wc.hCursor = LoadCursorW(None, IDC_ARROW).unwrap();
            let _ = RegisterClassExW(&wc);
        }

        let dark = theme::active_dark();
        let mut state = MessageDialogState {
            hwnd_text: HWND::default(),
            palette: DialogPalette::new(dark),
            buttons,
            result: None,
            completed: false,
        };

        let dpi = GetDpiForWindow(hwnd_parent).max(96) as i32;
        let scale = |value: i32| (value * dpi) / 96;
        let mut parent_rect = RECT::default();
        let _ = GetWindowRect(hwnd_parent, &mut parent_rect);
        let parent_w = parent_rect.right - parent_rect.left;
        let parent_h = parent_rect.bottom - parent_rect.top;
        let outer_w = scale(620).min((parent_w - scale(40)).max(scale(420)));
        let outer_h = scale(400).min((parent_h - scale(40)).max(scale(260)));
        let x = parent_rect.left + (parent_w - outer_w) / 2;
        let y = parent_rect.top + (parent_h - outer_h) / 2;

        let title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let hwnd_dlg = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_TOPMOST,
            MESSAGE_CLASS,
            PCWSTR(title_utf16.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            outer_w,
            outer_h,
            hwnd_parent,
            None,
            hinstance,
            None,
        )
        .unwrap();
        SetWindowLongPtrW(hwnd_dlg, GWLP_USERDATA, &mut state as *mut _ as isize);

        let mut client = RECT::default();
        let _ = GetClientRect(hwnd_dlg, &mut client);
        let pad = scale(14);
        let button_h = scale(28);
        let button_w = scale(84);
        let gap = scale(8);
        let text_w = (client.right - pad * 2).max(scale(120));
        let text_h = (client.bottom - pad * 2 - button_h - gap).max(scale(80));

        let message_utf16: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        state.hwnd_text = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(message_utf16.as_ptr()),
            WS_CHILD
                | WS_VISIBLE
                | WS_VSCROLL
                | WINDOW_STYLE(ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32),
            pad,
            pad,
            text_w,
            text_h,
            hwnd_dlg,
            None,
            hinstance,
            None,
        )
        .unwrap();

        let specs: &[(&str, i32)] = match buttons {
            MessageButtons::Ok => &[("OK", IDOK.0)],
            MessageButtons::YesNo => &[("Yes", IDYES.0), ("No", IDNO.0)],
            MessageButtons::YesNoCancel => {
                &[("Yes", IDYES.0), ("No", IDNO.0), ("Cancel", IDCANCEL.0)]
            }
        };
        let total_w = button_w * specs.len() as i32 + gap * (specs.len() as i32 - 1);
        let start_x = client.right - pad - total_w;
        for (index, (label, id)) in specs.iter().enumerate() {
            let mut label_utf16: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            let style = if index == 0 {
                BS_DEFPUSHBUTTON
            } else {
                BS_PUSHBUTTON
            };
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(label_utf16.as_mut_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(style as u32),
                start_x + index as i32 * (button_w + gap),
                client.bottom - pad - button_h,
                button_w,
                button_h,
                hwnd_dlg,
                HMENU(*id as u32 as usize as *mut _),
                hinstance,
                None,
            );
        }

        if !theme::active_high_contrast() {
            theme::apply_dialog_theme(hwnd_dlg, dark);
        }
        let _ = ShowWindow(hwnd_dlg, SW_SHOW);
        let _ = EnableWindow(hwnd_parent, false);
        let _ = SetFocus(state.hwnd_text);

        let mut msg = MSG::default();
        while !state.completed {
            let res = GetMessageW(&mut msg, None, 0, 0);
            if res.0 <= 0 {
                break;
            }
            if msg.message == WM_KEYDOWN {
                if msg.wParam.0 == 0x0D {
                    state.result = Some(match buttons {
                        MessageButtons::Ok => MessageResult::Ok,
                        MessageButtons::YesNo | MessageButtons::YesNoCancel => MessageResult::Yes,
                    });
                    state.completed = true;
                    let _ = DestroyWindow(hwnd_dlg);
                    break;
                } else if msg.wParam.0 == 0x1B {
                    state.result = Some(cancel_result(buttons));
                    state.completed = true;
                    let _ = DestroyWindow(hwnd_dlg);
                    break;
                }
            }
            if !IsDialogMessageW(hwnd_dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = EnableWindow(hwnd_parent, true);
        let _ = SetFocus(hwnd_parent);

        state.result.unwrap_or_else(|| cancel_result(buttons))
    }
}

pub fn show_error(hwnd: HWND, title: &str, message: &str) {
    let _ = show_message_box(
        hwnd,
        title,
        message,
        MessageButtons::Ok,
        MB_OK | MB_ICONERROR,
    );
}

pub fn show_info(hwnd: HWND, title: &str, message: &str) {
    let _ = show_message_box(
        hwnd,
        title,
        message,
        MessageButtons::Ok,
        MB_OK | MB_ICONINFORMATION,
    );
}

pub fn confirm_recycle(hwnd: HWND, item_count: usize) -> bool {
    let msg = format!(
        "Are you sure you want to move {} item{} to the Recycle Bin?",
        item_count,
        if item_count == 1 { "" } else { "s" }
    );
    show_message_box(
        hwnd,
        "Confirm Recycle",
        &msg,
        MessageButtons::YesNo,
        MB_YESNO | MB_ICONWARNING,
    ) == MessageResult::Yes
}

pub fn refuse_permanent_delete(hwnd: HWND) {
    let msg = "Permanent deletion (Shift+Delete) is refused by policy.\nAll file deletions must proceed through the Recycle Bin to safeguard data.";
    let _ = show_message_box(
        hwnd,
        "Permanent Deletion Refused",
        msg,
        MessageButtons::Ok,
        MB_OK | MB_ICONWARNING,
    );
}

struct PromptState {
    hwnd_edit: HWND,
    result: Option<String>,
    completed: bool,
    palette: DialogPalette,
}

pub fn prompt_input(
    hwnd_parent: HWND,
    title: &str,
    prompt: &str,
    default_value: &str,
) -> Option<String> {
    use windows::Win32::Foundation::{HINSTANCE, RECT};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};

    let _guard = modal_scope();

    const PROMPT_CLASS: PCWSTR = w!("RustExplorerPromptDialog");
    const IDC_PROMPT_EDIT: isize = 101;
    const IDC_PROMPT_OK: isize = 1;
    const IDC_PROMPT_CANCEL: isize = 2;

    unsafe extern "system" fn prompt_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PromptState;

        match msg {
            WM_ERASEBKGND => {
                if let Some(state) = state_ptr.as_ref() {
                    let mut rect = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rect);
                    let _ = FillRect(
                        HDC(wparam.0 as *mut _),
                        &rect,
                        state.palette.background_brush(),
                    );
                    return LRESULT(1);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                if let Some(state) = state_ptr.as_ref() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, state.palette.text_color());
                    let _ = SetBkColor(hdc, state.palette.background_color());
                    let _ = SetBkMode(hdc, TRANSPARENT);
                    return LRESULT(state.palette.background_brush().0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT => {
                if let Some(state) = state_ptr.as_ref() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, state.palette.text_color());
                    let _ = SetBkColor(hdc, state.palette.input_color());
                    return LRESULT(state.palette.input_brush().0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as isize;
                if let Some(state) = state_ptr.as_mut() {
                    if id == IDC_PROMPT_OK {
                        let len = GetWindowTextLengthW(state.hwnd_edit) as usize;
                        let mut buf = vec![0u16; len + 1];
                        let copied = GetWindowTextW(state.hwnd_edit, &mut buf) as usize;
                        let text = String::from_utf16_lossy(&buf[..copied]);
                        state.result = Some(text);
                        state.completed = true;
                        let _ = DestroyWindow(hwnd);
                        return LRESULT(0);
                    } else if id == IDC_PROMPT_CANCEL {
                        state.completed = true;
                        let _ = DestroyWindow(hwnd);
                        return LRESULT(0);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                if let Some(state) = state_ptr.as_mut() {
                    state.completed = true;
                }
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap().0);

        let mut wc = WNDCLASSEXW::default();
        if GetClassInfoExW(hinstance, PROMPT_CLASS, &mut wc).is_err() {
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.lpfnWndProc = Some(prompt_wnd_proc);
            wc.hInstance = hinstance;
            wc.lpszClassName = PROMPT_CLASS;
            wc.hCursor = LoadCursorW(None, IDC_ARROW).unwrap();
            let _ = RegisterClassExW(&wc);
        }

        let mut state = PromptState {
            hwnd_edit: HWND(std::ptr::null_mut()),
            result: None,
            completed: false,
            palette: DialogPalette::new(theme::active_dark()),
        };

        let dpi = GetDpiForWindow(hwnd_parent).max(96) as i32;
        let scale = |value: i32| (value * dpi) / 96;
        let mut parent_rect = RECT::default();
        let _ = GetWindowRect(hwnd_parent, &mut parent_rect);
        let dlg_w = scale(420);
        let dlg_h = scale(170);
        let dlg_x = parent_rect.left + ((parent_rect.right - parent_rect.left) - dlg_w) / 2;
        let dlg_y = parent_rect.top + ((parent_rect.bottom - parent_rect.top) - dlg_h) / 2;

        let title_utf16: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();

        let hwnd_dlg = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_TOPMOST,
            PROMPT_CLASS,
            PCWSTR(title_utf16.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            dlg_x,
            dlg_y,
            dlg_w,
            dlg_h,
            hwnd_parent,
            None,
            hinstance,
            None,
        )
        .unwrap();

        SetWindowLongPtrW(
            hwnd_dlg,
            GWLP_USERDATA,
            &mut state as *mut PromptState as isize,
        );

        let prompt_utf16: Vec<u16> = prompt.encode_utf16().chain(Some(0)).collect();
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            PCWSTR(prompt_utf16.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            scale(20),
            scale(15),
            scale(380),
            scale(20),
            hwnd_dlg,
            None,
            hinstance,
            None,
        );

        let def_utf16: Vec<u16> = default_value.encode_utf16().chain(Some(0)).collect();
        state.hwnd_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(def_utf16.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            scale(20),
            scale(45),
            scale(365),
            scale(25),
            hwnd_dlg,
            HMENU(IDC_PROMPT_EDIT as *mut _),
            hinstance,
            None,
        )
        .unwrap();

        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("OK"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            scale(215),
            scale(85),
            scale(80),
            scale(28),
            hwnd_dlg,
            HMENU(IDC_PROMPT_OK as *mut _),
            hinstance,
            None,
        );

        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Cancel"),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            scale(305),
            scale(85),
            scale(80),
            scale(28),
            hwnd_dlg,
            HMENU(IDC_PROMPT_CANCEL as *mut _),
            hinstance,
            None,
        );

        if !theme::active_high_contrast() {
            theme::apply_dialog_theme(hwnd_dlg, state.palette.dark);
        }
        let _ = ShowWindow(hwnd_dlg, SW_SHOW);

        let _ = SendMessageW(
            state.hwnd_edit,
            EM_SETSEL,
            WPARAM(0),
            LPARAM(default_value.len() as isize),
        );
        let _ = SetFocus(state.hwnd_edit);
        let _ = EnableWindow(hwnd_parent, false);

        let mut msg = MSG::default();
        while !state.completed {
            let res = GetMessageW(&mut msg, None, 0, 0);
            if res.0 <= 0 {
                break;
            }
            if msg.message == WM_KEYDOWN {
                if msg.wParam.0 == 0x0D {
                    // Enter
                    let len = GetWindowTextLengthW(state.hwnd_edit) as usize;
                    let mut buf = vec![0u16; len + 1];
                    let copied = GetWindowTextW(state.hwnd_edit, &mut buf) as usize;
                    let text = String::from_utf16_lossy(&buf[..copied]);
                    state.result = Some(text);
                    state.completed = true;
                    let _ = DestroyWindow(hwnd_dlg);
                    break;
                } else if msg.wParam.0 == 0x1B {
                    // Escape
                    state.completed = true;
                    let _ = DestroyWindow(hwnd_dlg);
                    break;
                }
            }
            if !IsDialogMessageW(hwnd_dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = EnableWindow(hwnd_parent, true);
        let _ = SetFocus(hwnd_parent);

        state.result
    }
}

/// Modal dialog showing Indexed Roots status, with recrawl and remove buttons (P11).
pub fn show_roots_manager_dialog(
    hwnd_parent: HWND,
    app_state: Arc<AppState>,
    runtime: tokio::runtime::Handle,
    current_folder_token: Option<explorer_domain::ids::FolderToken>,
) {
    let _guard = modal_scope();
    let roots = match app_state.index_service.list_roots() {
        Ok(r) => r,
        Err(e) => {
            show_error(
                hwnd_parent,
                "Indexed Roots",
                &format!("Failed to list roots: {e}"),
            );
            return;
        }
    };

    let mut message = String::from("Currently Indexed Roots:\n\n");
    if roots.is_empty() {
        message.push_str("No roots are currently indexed.\n");
    } else {
        for (i, root) in roots.iter().enumerate() {
            message.push_str(&format!(
                "{}. {} [{:?}] (ID: {})\n",
                i + 1,
                root.path.display(),
                root.state,
                &root.id[..root.id.len().min(8)]
            ));
        }
    }

    message.push_str("\nOptions:\n[Yes] Add current directory to indexed roots\n[No] Recrawl all roots\n[Cancel] Close");

    let title = "Indexed Roots Manager";
    let result = show_message_box(
        hwnd_parent,
        title,
        &message,
        MessageButtons::YesNoCancel,
        MB_YESNOCANCEL | MB_ICONINFORMATION,
    );

    if result == MessageResult::Yes {
        if let Some(token) = current_folder_token {
            let app = app_state.clone();
            let runtime = runtime.clone();
            runtime.spawn(async move {
                let _ = explorer_app::operations::add_indexed_root(&app, &token).await;
            });
            show_info(
                hwnd_parent,
                "Indexed Roots",
                "Indexing initiated for current directory.",
            );
        } else {
            show_error(
                hwnd_parent,
                "Indexed Roots",
                "No folder currently active to index.",
            );
        }
    } else if result == MessageResult::No {
        for root in roots {
            let app = app_state.clone();
            let rid = root.id.clone();
            let runtime = runtime.clone();
            runtime.spawn(async move {
                let _ = explorer_app::operations::recrawl_indexed_root(&app, &rid).await;
            });
        }
        show_info(
            hwnd_parent,
            "Indexed Roots",
            "Recrawl initiated for all roots.",
        );
    }
}

/// Modal dialog showing recent file operation jobs history (P21).
pub fn show_activity_dialog(hwnd_parent: HWND, app_state: Arc<AppState>) {
    let _guard = modal_scope();
    let jobs = match app_state.operation_service.list_recent_jobs(20) {
        Ok(j) => j,
        Err(e) => {
            show_error(
                hwnd_parent,
                "Activity Log",
                &format!("Failed to load jobs: {e}"),
            );
            return;
        }
    };

    let mut message = String::from("Recent File Operations:\n\n");
    if jobs.is_empty() {
        message.push_str("No recorded operations in this session.\n");
    } else {
        for (i, job) in jobs.iter().enumerate() {
            message.push_str(&format!(
                "{}. {:?} - {:?} (items: {})\n",
                i + 1,
                job.kind,
                job.state,
                job.total_items
            ));
            if let Some(err) = &job.error_message {
                message.push_str(&format!("   Error: {}\n", err));
            }
        }
    }

    show_info(hwnd_parent, "Activity & Job History", &message);
}
