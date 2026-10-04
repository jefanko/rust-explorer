//! Per-user state-directory lease. Acquire before opening databases or recovering jobs.
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, GetWindowThreadProcessId, IsWindow, SW_RESTORE, SetForegroundWindow,
    ShowWindowAsync,
};

pub struct InstanceLease(File);

impl InstanceLease {
    /// Returns None for an existing owner; every other error fails startup closed.
    pub fn acquire(state_dir: &Path) -> io::Result<Option<Self>> {
        if !state_dir.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "State directory must be absolute",
            ));
        }
        std::fs::create_dir_all(state_dir)?;
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(1) // FILE_SHARE_READ: observers can read HWND, no other writer/deleter.
            .open(state_dir.join("instance.lock"))
        {
            Ok(file) => {
                file.set_len(0)?;
                Ok(Some(Self(file)))
            }
            Err(error) if error.raw_os_error() == Some(32) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn publish_window(&mut self, window_handle: usize) -> io::Result<()> {
        let hwnd = HWND(window_handle as *mut _);
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid != std::process::id() || !unsafe { IsWindow(hwnd) }.as_bool() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Window is not owned by this process",
            ));
        }
        self.0.seek(SeekFrom::Start(0))?;
        self.0.set_len(0)?;
        write!(self.0, "{} {}", pid, hwnd.0 as usize)?;
        self.0.sync_all()
    }
}

pub fn show_startup_error(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::{PCWSTR, w};
    let message: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(message.as_ptr()),
            w!("Rust Explorer — startup failed"),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// No command-line navigation or mutation intent is forwarded.
pub fn focus_existing(state_dir: &Path) -> bool {
    // A second launch can race the primary's window creation.
    for _ in 0..40 {
        if let Ok(record) = std::fs::read_to_string(state_dir.join("instance.lock")) {
            let mut fields = record.split_whitespace();
            if let (Some(pid), Some(hwnd)) = (
                fields.next().and_then(|s| s.parse::<u32>().ok()),
                fields.next().and_then(|s| s.parse::<usize>().ok()),
            ) {
                let hwnd = HWND(hwnd as *mut _);
                let mut actual_pid = 0;
                unsafe {
                    GetWindowThreadProcessId(hwnd, Some(&mut actual_pid));
                }
                if actual_pid == pid && unsafe { IsWindow(hwnd) }.as_bool() {
                    unsafe {
                        let _ = AllowSetForegroundWindow(pid);
                        let _ = ShowWindowAsync(hwnd, SW_RESTORE);
                        return SetForegroundWindow(hwnd).as_bool();
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instance_lease_child_probe() {
        let Some(root) = std::env::var_os("RUST_EXPLORER_LEASE_FIXTURE") else {
            return;
        };
        let lease = InstanceLease::acquire(Path::new(&root)).unwrap();
        assert_eq!(
            lease.is_none(),
            std::env::var("RUST_EXPLORER_LEASE_EXPECT_HELD").unwrap() == "yes"
        );
    }
    #[test]
    fn instance_lease_is_cross_process_and_does_not_touch_journals() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".rust-explorer-fixture-root"),
            "cross process lease fixture",
        )
        .unwrap();
        let sentinel = root.path().join("state.sqlite3");
        std::fs::write(&sentinel, "unfinished journal sentinel").unwrap();
        let lease = InstanceLease::acquire(root.path()).unwrap().unwrap();
        let child = |held: &str| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "instance::tests::instance_lease_child_probe",
                    "--test-threads=1",
                ])
                .env("RUST_EXPLORER_LEASE_FIXTURE", root.path())
                .env("RUST_EXPLORER_LEASE_EXPECT_HELD", held)
                .output()
                .unwrap()
        };
        let denied = child("yes");
        assert!(
            denied.status.success(),
            "{}",
            String::from_utf8_lossy(&denied.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(&sentinel).unwrap(),
            "unfinished journal sentinel"
        );
        drop(lease);
        assert!(child("no").status.success());
    }

    #[test]
    fn instance_lease_excludes_competitors_and_allows_relaunch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".rust-explorer-fixture-root"),
            "instance fixture",
        )
        .unwrap();
        let lease = InstanceLease::acquire(root.path()).unwrap().unwrap();
        assert!(InstanceLease::acquire(root.path()).unwrap().is_none());
        assert!(std::fs::read_to_string(root.path().join("instance.lock")).is_ok());
        drop(lease);
        assert!(InstanceLease::acquire(root.path()).unwrap().is_some());
        let unavailable = root.path().join("unavailable");
        std::fs::write(&unavailable, "file").unwrap();
        assert!(InstanceLease::acquire(&unavailable).is_err());
    }
}
