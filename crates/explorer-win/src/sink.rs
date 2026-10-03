use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::UI::Shell::{
    IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
};
use windows::core::{HRESULT, PCWSTR, Result, implement};

/// Transfer source flag indicating if the Shell allows recycling (0x00000080).
const TSF_DELETE_RECYCLE_IF_POSSIBLE: u32 = 0x00000080;

#[derive(Debug, Clone, Default)]
pub struct SinkReport {
    pub completed: usize,
    pub failed: usize,
    pub was_aborted: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, Default)]
pub struct SinkTracker {
    pub completed_count: Arc<AtomicUsize>,
    pub failed_count: Arc<AtomicUsize>,
    pub aborted: Arc<AtomicBool>,
    pub last_error: Arc<Mutex<Option<String>>>,
}

impl SinkTracker {
    pub fn report(&self) -> SinkReport {
        SinkReport {
            completed: self.completed_count.load(Ordering::SeqCst),
            failed: self.failed_count.load(Ordering::SeqCst),
            was_aborted: self.aborted.load(Ordering::SeqCst),
            error_message: self.last_error.lock().unwrap().clone(),
        }
    }
}

#[implement(IFileOperationProgressSink)]
pub struct ShellProgressSink {
    recycle_only: bool,
    tracker: SinkTracker,
}

impl ShellProgressSink {
    pub fn new(recycle_only: bool) -> (Self, SinkTracker) {
        let tracker = SinkTracker::default();
        (
            Self {
                recycle_only,
                tracker: tracker.clone(),
            },
            tracker,
        )
    }
}

#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for ShellProgressSink_Impl {
    fn StartOperations(&self) -> Result<()> {
        Ok(())
    }

    fn FinishOperations(&self, hr: HRESULT) -> Result<()> {
        if hr.is_err() && self.tracker.last_error.lock().unwrap().is_none() {
            *self.tracker.last_error.lock().unwrap() = Some(format!(
                "Operations finished with HRESULT 0x{:08X}",
                hr.0 as u32
            ));
        }
        Ok(())
    }

    fn PreRenameItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn PostRenameItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        hr: HRESULT,
        _psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        if hr.is_ok() {
            self.tracker.completed_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.tracker.failed_count.fetch_add(1, Ordering::SeqCst);
            *self.tracker.last_error.lock().unwrap() =
                Some(format!("Rename item failed: 0x{:08X}", hr.0 as u32));
        }
        Ok(())
    }

    fn PreMoveItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn PostMoveItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        hr: HRESULT,
        _psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        if hr.is_ok() {
            self.tracker.completed_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.tracker.failed_count.fetch_add(1, Ordering::SeqCst);
            *self.tracker.last_error.lock().unwrap() =
                Some(format!("Move item failed: 0x{:08X}", hr.0 as u32));
        }
        Ok(())
    }

    fn PreCopyItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn PostCopyItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        hr: HRESULT,
        _psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        if hr.is_ok() {
            self.tracker.completed_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.tracker.failed_count.fetch_add(1, Ordering::SeqCst);
            *self.tracker.last_error.lock().unwrap() =
                Some(format!("Copy item failed: 0x{:08X}", hr.0 as u32));
        }
        Ok(())
    }

    fn PreDeleteItem(&self, dwflags: u32, _psiitem: Option<&IShellItem>) -> Result<()> {
        // Strict safety rule: if recycle_only is true, verify that the Shell allows recycling.
        // Never silently fall back from Recycle to permanent deletion!
        if self.recycle_only && (dwflags & TSF_DELETE_RECYCLE_IF_POSSIBLE) == 0 {
            self.tracker.aborted.store(true, Ordering::SeqCst);
            *self.tracker.last_error.lock().unwrap() =
                Some("Item cannot be recycled; permanent deletion was prevented.".to_string());
            return Err(windows::core::Error::from_hresult(E_FAIL));
        }
        Ok(())
    }

    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        _psiitem: Option<&IShellItem>,
        hr: HRESULT,
        _psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        if hr.is_ok() {
            self.tracker.completed_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.tracker.failed_count.fetch_add(1, Ordering::SeqCst);
            *self.tracker.last_error.lock().unwrap() =
                Some(format!("Delete item failed: 0x{:08X}", hr.0 as u32));
        }
        Ok(())
    }

    fn PreNewItem(
        &self,
        _dwflags: u32,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn PostNewItem(
        &self,
        _dwflags: u32,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        _psztemplate: &PCWSTR,
        _dwattr: u32,
        hr: HRESULT,
        _psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        if hr.is_ok() {
            self.tracker.completed_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.tracker.failed_count.fetch_add(1, Ordering::SeqCst);
            *self.tracker.last_error.lock().unwrap() =
                Some(format!("Create item failed: 0x{:08X}", hr.0 as u32));
        }
        Ok(())
    }

    fn UpdateProgress(&self, _worktotal: u32, _worksofar: u32) -> Result<()> {
        Ok(())
    }

    fn ResetTimer(&self) -> Result<()> {
        Ok(())
    }

    fn PauseTimer(&self) -> Result<()> {
        Ok(())
    }

    fn ResumeTimer(&self) -> Result<()> {
        Ok(())
    }
}
