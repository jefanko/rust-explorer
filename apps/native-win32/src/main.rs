#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod controls;
mod diagnostics;
mod dialogs;
mod dispatcher;
mod icons;
mod presenter;
mod preview;
mod resources;
mod sidebar;
mod window;

use controls::*;
use explorer_app::lifecycle::initialize_application;
use explorer_app::state::application_state_dir;
use explorer_domain::ids::SessionId;
use std::path::PathBuf;
use std::sync::Arc;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES,
    ICC_TREEVIEW_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn main() {
    diagnostics::init();
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Rust Explorer Native Win32 host");

    // Initialize COM Single-Threaded Apartment on the UI thread
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    // Initialize Common Controls v6 classes
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES
            | ICC_LISTVIEW_CLASSES
            | ICC_BAR_CLASSES
            | ICC_TAB_CLASSES
            | ICC_TREEVIEW_CLASSES,
    };
    unsafe {
        let _ = InitCommonControlsEx(&icc);
    }

    // Initialize durable state with fail-closed error handling
    let state_dir = match application_state_dir() {
        Ok(dir) => dir,
        Err(error) => {
            tracing::error!("Startup failed: {error}");
            explorer_win::instance::show_startup_error(&format!(
                "Rust Explorer could not initialize durable state.\n\n{error}\n\nNo mutation executor was made available."
            ));
            return;
        }
    };

    let startup = match initialize_application(&state_dir) {
        Ok(Some(startup)) => startup,
        Ok(None) => {
            tracing::info!("Secondary launch detected; existing window focused. Exiting cleanly.");
            return;
        }
        Err(error) => {
            tracing::error!("Startup failed: {error}");
            explorer_win::instance::show_startup_error(&format!(
                "Rust Explorer could not initialize durable state.\n\n{error}\n\nNo mutation executor was made available."
            ));
            return;
        }
    };

    let (instance_lease, app_state) = startup;
    let app_state = Arc::new(app_state);

    // Explicitly owned Tokio background runtime (capped at 2 worker threads)
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    // Restore indexed root watches and crawls
    let index_svc = app_state.index_service.clone();
    let watch_svc = app_state.watch_service.clone();
    rt.spawn(async move {
        explorer_app::lifecycle::restore_indexed_roots(index_svc, watch_svc).await;
    });

    // Bootstrap initial path
    let bootstrap_data = rt
        .block_on(explorer_app::operations::bootstrap(&app_state))
        .unwrap_or_else(|_| explorer_domain::models::BootstrapData {
            session_id: SessionId::new(),
            known_folders: Vec::new(),
            drives: Vec::new(),
            initial_path: r"C:\".to_string(),
        });

    let initial_path = PathBuf::from(&bootstrap_data.initial_path);
    let session_id = bootstrap_data.session_id;

    let hinstance = unsafe { HINSTANCE(GetModuleHandleW(None).unwrap().0) };

    let hwnd = match window::create_main_window(
        hinstance,
        app_state.clone(),
        rt.handle().clone(),
        session_id,
        initial_path,
    ) {
        Ok(h) => h,
        Err(err) => {
            tracing::error!("Failed to create main window: {err}");
            return;
        }
    };

    // Publish authorized window HWND to the per-user instance lease
    if let Ok(mut lease) = instance_lease.lock() {
        let _ = lease.publish_window(hwnd.0 as usize);
    }

    // Accelerators for native shortcuts (P26)
    let accels = [
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'L' as u16,
            cmd: ID_ACTION_FOCUS_ADDRESS,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: b'D' as u16,
            cmd: ID_ACTION_FOCUS_ADDRESS,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'F' as u16,
            cmd: ID_ACTION_FOCUS_SEARCH,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'T' as u16,
            cmd: ID_ACTION_NEW_TAB,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'W' as u16,
            cmd: ID_ACTION_CLOSE_TAB,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_TAB.0,
            cmd: ID_ACTION_NEXT_TAB,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_TAB.0,
            cmd: ID_ACTION_PREV_TAB,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'A' as u16,
            cmd: ID_ACTION_SELECT_ALL,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'C' as u16,
            cmd: ID_ACTION_COPY,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'X' as u16,
            cmd: ID_ACTION_CUT,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'V' as u16,
            cmd: ID_ACTION_PASTE,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'P' as u16,
            cmd: ID_ACTION_TOGGLE_PREVIEW,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: b'P' as u16,
            cmd: ID_ACTION_TOGGLE_PREVIEW,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: b'N' as u16,
            cmd: ID_ACTION_NEW_FOLDER,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_F2.0,
            cmd: ID_ACTION_RENAME,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_DELETE.0,
            cmd: ID_ACTION_RECYCLE,
        },
        ACCEL {
            fVirt: FVIRTKEY | FSHIFT,
            key: VK_DELETE.0,
            cmd: ID_ACTION_PERMANENT_DELETE_REFUSAL,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_F5.0,
            cmd: IDC_NAV_REFRESH,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: VK_LEFT.0,
            cmd: IDC_NAV_BACK,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: VK_RIGHT.0,
            cmd: IDC_NAV_FORWARD,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: VK_UP.0,
            cmd: IDC_NAV_UP,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: VK_RETURN.0,
            cmd: ID_ACTION_PROPERTIES,
        },
    ];

    let haccel = unsafe { CreateAcceleratorTableW(&accels).unwrap() };

    // Standard Win32 GetMessage loop
    unsafe {
        let mut msg = MSG::default();
        loop {
            let res = GetMessageW(&mut msg, None, 0, 0);
            if res.0 == 0 {
                // WM_QUIT received
                break;
            } else if res.0 == -1 {
                tracing::error!("GetMessageW returned error -1");
                break;
            } else {
                if TranslateAcceleratorW(hwnd, haccel, &msg) == 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        let _ = DestroyAcceleratorTable(haccel);
        CoUninitialize();
    }

    // Do not block process exit on crawl/watch/blocking tasks: they are bound to
    // the window lifetime and must be abandoned once the message loop ends.
    rt.shutdown_background();

    tracing::info!("Rust Explorer Native host exited cleanly");
}
