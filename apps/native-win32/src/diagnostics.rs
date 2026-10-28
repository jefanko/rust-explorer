//! Crash, panic, and protocol diagnostics for the native host.
//!
//! A GUI subsystem binary has no console, so every failure that used to vanish
//! (Rust panic, access violation in a common control, unhandled exception in a
//! window callback) is written to a durable log next to the application state.

#![allow(unsafe_op_in_unsafe_fn)]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::Diagnostics::Debug::{
    EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS, RtlCaptureStackBackTrace, SYMBOL_INFOW,
    SYMOPT_DEFERRED_LOADS, SYMOPT_LOAD_LINES, SYMOPT_UNDNAME, SetUnhandledExceptionFilter,
    SymFromAddrW, SymGetOptions, SymInitializeW, SymSetOptions,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId};
use windows::core::PCWSTR;

static LOG: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

fn log_lock() -> &'static Mutex<Option<File>> {
    LOG.get_or_init(|| Mutex::new(None))
}

fn log_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("RustExplorer")
        .join("logs");
    let _ = std::fs::create_dir_all(&base);
    base.join("rust-explorer.log")
}

/// Appends one line to the crash log and flushes immediately (crash-safe).
pub fn write_line(message: &str) {
    let mut guard = match log_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.is_none() {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .ok();
        *guard = file;
    }
    if let Some(file) = guard.as_mut() {
        let _ = writeln!(file, "{message}");
        let _ = file.flush();
    }
}

/// Enables per-message tracing (`RUST_EXPLORER_TRACE=1`).
pub fn trace_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::Relaxed)
}

/// Records one window-message breadcrumb when tracing is enabled.
pub fn trace_message(msg: u32, wparam: usize, lparam: isize, detail: &str) {
    if !trace_enabled() {
        return;
    }
    write_line(&format!(
        "msg {msg:#06x} wp={wparam:#x} lp={lparam:#x} {detail}"
    ));
}

fn module_for(address: usize) -> (String, u64) {
    unsafe {
        let mut module = HMODULE::default();
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            PCWSTR(address as *const u16),
            &mut module,
        )
        .is_err()
        {
            return ("<unknown>".to_string(), 0);
        }
        let mut buffer = [0u16; 600];
        let len = GetModuleFileNameW(module, &mut buffer) as usize;
        let name = String::from_utf16_lossy(&buffer[..len]);
        let short = name.rsplit('\\').next().unwrap_or(&name).to_string();
        (short, module.0 as u64)
    }
}

fn format_frame(index: usize, address: usize) -> String {
    let (module, base) = module_for(address);
    let offset = address as u64 - base;
    let mut symbol = resolve_symbol(address as u64);
    if symbol.is_empty() {
        symbol = format!("{module}+{offset:#x}");
    } else {
        symbol = format!("{module}!{symbol}");
    }
    format!("  #{index:02} {address:#018x} {symbol}")
}

fn resolve_symbol(address: u64) -> String {
    static SYMBOLS_READY: OnceLock<bool> = OnceLock::new();
    let ready = *SYMBOLS_READY.get_or_init(|| unsafe {
        SymSetOptions(SymGetOptions() | SYMOPT_DEFERRED_LOADS | SYMOPT_UNDNAME | SYMOPT_LOAD_LINES);
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.to_path_buf()));
        let search_path: Vec<u16> = exe_dir
            .as_ref()
            .map(|dir| dir.as_os_str().encode_wide().chain(Some(0)).collect())
            .unwrap_or_default();
        let search = if search_path.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(search_path.as_ptr())
        };
        SymInitializeW(GetCurrentProcess(), search, true).is_ok()
    });
    if !ready {
        return String::new();
    }
    // SYMBOL_INFOW ends in a one-element name array; over-allocate a real buffer.
    let mut raw = vec![0u64; (std::mem::size_of::<SYMBOL_INFOW>() + 1024 + 7) / 8];
    let symbol = raw.as_mut_ptr() as *mut SYMBOL_INFOW;
    unsafe {
        (*symbol).SizeOfStruct = std::mem::size_of::<SYMBOL_INFOW>() as u32;
        (*symbol).MaxNameLen = 512;
        let mut displacement = 0u64;
        if SymFromAddrW(
            GetCurrentProcess(),
            address,
            Some(&mut displacement),
            symbol,
        )
        .is_err()
        {
            return String::new();
        }
        let name_ptr = (*symbol).Name.as_ptr();
        let mut len = 0usize;
        while len < 512 && *name_ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, len))
    }
}

/// Formats the current native stack (used from panic and crash handlers).
fn stack_backtrace(skip: u32) -> String {
    let mut frames = [std::ptr::null_mut::<core::ffi::c_void>(); 64];
    let count = unsafe { RtlCaptureStackBackTrace(skip, &mut frames, None) } as usize;
    let mut out = String::from("stack:\n");
    for (index, frame) in frames[..count].iter().enumerate() {
        out.push_str(&format_frame(index, *frame as usize));
        out.push('\n');
    }
    out
}

/// Installs the panic hook: a Rust panic no longer aborts silently.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        write_line(&format!(
            "[panic] pid={} thread={:?} at {location}: {payload}\n{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("<unnamed>"),
            stack_backtrace(2)
        ));
        default_hook(info);
    }));
}

unsafe extern "system" fn crash_filter(info: *const EXCEPTION_POINTERS) -> i32 {
    let mut code = 0u32;
    let mut address = 0usize;
    if !info.is_null() && !(*info).ExceptionRecord.is_null() {
        code = (*(*info).ExceptionRecord).ExceptionCode.0 as u32;
        address = (*(*info).ExceptionRecord).ExceptionAddress as usize;
    }
    write_line(&format!(
        "[crash] pid={} code={code:#010x} address={address:#018x}\n{}",
        std::process::id(),
        stack_backtrace(0)
    ));
    // Let Windows Error Reporting keep its normal behaviour after logging.
    EXCEPTION_CONTINUE_SEARCH
}

/// Installs the process-wide unhandled-exception logger.
pub fn install_crash_handler() {
    if let Some(path) = std::env::var_os("RUST_EXPLORER_TRACE") {
        TRACE_ENABLED.store(!path.is_empty(), Ordering::Relaxed);
    }
    unsafe {
        SetUnhandledExceptionFilter(Some(crash_filter));
    }
}

/// Full initialization used by `main` before any window is created.
pub fn init() {
    install_panic_hook();
    install_crash_handler();
    write_line(&format!("=== rust-explorer start pid={} ===", unsafe {
        GetCurrentProcessId()
    }));
}
