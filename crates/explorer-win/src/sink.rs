use explorer_domain::operations::ItemStatus;
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{E_FAIL, S_OK};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::COPYENGINE_S_DONT_PROCESS_CHILDREN;
use windows::Win32::UI::Shell::{
    COPYENGINE_E_CANCELLED, COPYENGINE_E_USER_CANCELLED, COPYENGINE_S_COLLISIONRESOLVED,
    COPYENGINE_S_KEEP_BOTH, COPYENGINE_S_MERGE, IFileOperationProgressSink,
    IFileOperationProgressSink_Impl, IShellItem, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, Interface, PCWSTR, Result, implement};

pub type SinkItemCallback = Arc<dyn Fn(SinkItemResult) + Send + Sync>;

/// Transfer source flag indicating if the Shell allows recycling (0x00000080).
const TSF_DELETE_RECYCLE_IF_POSSIBLE: u32 = 0x00000080;

#[derive(Debug, Clone, Default)]
pub struct SinkReport {
    pub completed: usize,
    pub failed: usize,
    pub was_aborted: bool,
    pub error_message: Option<String>,
    pub native_error: Option<u32>,
    pub item_results: Vec<SinkItemResult>,
}

#[derive(Debug, Clone)]
pub struct SinkItemResult {
    pub source_path: Option<PathBuf>,
    pub actual_destination: Option<PathBuf>,
    pub status: ItemStatus,
    pub native_code: Option<u32>,
    pub error_message: Option<String>,
}

#[derive(Clone, Default)]
pub struct SinkTracker {
    pub completed_count: Arc<AtomicUsize>,
    pub failed_count: Arc<AtomicUsize>,
    pub aborted: Arc<AtomicBool>,
    pub last_error: Arc<Mutex<Option<String>>>,
    pub item_results: Arc<Mutex<Vec<SinkItemResult>>>,
    item_callback: Option<SinkItemCallback>,
    pre_sources: Arc<Mutex<HashMap<usize, PathBuf>>>,
}

impl SinkTracker {
    /// Records a result returned by a Shell transfer provider (not a COM progress callback).
    pub fn record_provider_result(&self, hr: HRESULT, source: PathBuf, output: Option<PathBuf>) {
        self.record_hresult(hr, "Shell provider rename", Some(source), output);
    }
    fn remember_source(&self, item: Option<&IShellItem>) {
        if let Some(item) = item
            && let Some(path) = shell_path(Some(item))
        {
            self.pre_sources
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(item.as_raw() as usize, path);
        }
    }
    fn source_path(&self, item: Option<&IShellItem>) -> Option<PathBuf> {
        item.and_then(|item| {
            self.pre_sources
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&(item.as_raw() as usize))
                .or_else(|| shell_path(Some(item)))
        })
    }
    fn record_hresult(
        &self,
        hr: HRESULT,
        action: &str,
        source_path: Option<PathBuf>,
        actual_destination: Option<PathBuf>,
    ) {
        let status = if hr == COPYENGINE_S_DONT_PROCESS_CHILDREN
            && actual_destination.as_ref().is_some_and(|p| p.exists())
        {
            ItemStatus::Succeeded
        } else {
            classify_hresult(hr)
        };
        match status {
            ItemStatus::Succeeded => {
                self.completed_count.fetch_add(1, Ordering::SeqCst);
            }
            ItemStatus::Failed => {
                self.failed_count.fetch_add(1, Ordering::SeqCst);
            }
            ItemStatus::Canceled => {
                self.aborted.store(true, Ordering::SeqCst);
            }
            ItemStatus::Skipped => {}
        }
        let message = (status != ItemStatus::Succeeded)
            .then(|| format!("{action}: {status:?}, HRESULT 0x{:08X}", hr.0 as u32));
        if status == ItemStatus::Failed || status == ItemStatus::Canceled {
            *self.last_error.lock().unwrap_or_else(|e| e.into_inner()) = message.clone();
        }
        let result = SinkItemResult {
            source_path,
            actual_destination,
            status,
            native_code: Some(hr.0 as u32),
            error_message: message,
        };
        self.item_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(result.clone());
        if let Some(callback) = &self.item_callback {
            callback(result);
        }
    }

    pub fn report(&self) -> SinkReport {
        SinkReport {
            completed: self.completed_count.load(Ordering::SeqCst),
            failed: self.failed_count.load(Ordering::SeqCst),
            was_aborted: self.aborted.load(Ordering::SeqCst),
            error_message: self
                .last_error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            native_error: None,
            item_results: self
                .item_results
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
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
        Self::new_with_callback(recycle_only, None)
    }

    pub fn new_with_callback(
        recycle_only: bool,
        item_callback: Option<SinkItemCallback>,
    ) -> (Self, SinkTracker) {
        let tracker = SinkTracker {
            item_callback,
            ..SinkTracker::default()
        };
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
        if hr.is_err()
            && self
                .tracker
                .last_error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none()
        {
            *self
                .tracker
                .last_error
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(format!(
                "Operations finished with HRESULT 0x{:08X}",
                hr.0 as u32
            ));
        }
        Ok(())
    }

    fn PreRenameItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        self.tracker.remember_source(psiitem);
        Ok(())
    }

    fn PostRenameItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        hr: HRESULT,
        psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        self.tracker.record_hresult(
            hr,
            "Rename item",
            self.tracker.source_path(psiitem),
            shell_path(psinewitem),
        );
        Ok(())
    }

    fn PreMoveItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        self.tracker.remember_source(psiitem);
        Ok(())
    }

    fn PostMoveItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        hr: HRESULT,
        psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        self.tracker.record_hresult(
            hr,
            "Move item",
            self.tracker.source_path(psiitem),
            shell_path(psinewitem),
        );
        Ok(())
    }

    fn PreCopyItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
    ) -> Result<()> {
        self.tracker.remember_source(psiitem);
        Ok(())
    }

    fn PostCopyItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        _psidest: Option<&IShellItem>,
        _psznewname: &PCWSTR,
        hr: HRESULT,
        psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        self.tracker.record_hresult(
            hr,
            "Copy item",
            self.tracker.source_path(psiitem),
            shell_path(psinewitem),
        );
        Ok(())
    }

    fn PreDeleteItem(&self, dwflags: u32, psiitem: Option<&IShellItem>) -> Result<()> {
        self.tracker.remember_source(psiitem);
        // Strict safety rule: if recycle_only is true, verify that the Shell allows recycling.
        // Never silently fall back from Recycle to permanent deletion!
        if self.recycle_only && (dwflags & TSF_DELETE_RECYCLE_IF_POSSIBLE) == 0 {
            self.tracker.aborted.store(true, Ordering::SeqCst);
            self.tracker
                .record_hresult(E_FAIL, "Recycle safety check", shell_path(psiitem), None);
            *self
                .tracker
                .last_error
                .lock()
                .unwrap_or_else(|e| e.into_inner()) =
                Some("Item cannot be recycled; permanent deletion was prevented.".to_string());
            return Err(windows::core::Error::from_hresult(E_FAIL));
        }
        Ok(())
    }

    fn PostDeleteItem(
        &self,
        _dwflags: u32,
        psiitem: Option<&IShellItem>,
        hr: HRESULT,
        psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        self.tracker.record_hresult(
            hr,
            "Delete item",
            self.tracker.source_path(psiitem),
            shell_path(psinewitem),
        );
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
        psidest: Option<&IShellItem>,
        psznewname: &PCWSTR,
        _psztemplate: &PCWSTR,
        _dwattr: u32,
        hr: HRESULT,
        psinewitem: Option<&IShellItem>,
    ) -> Result<()> {
        let source = shell_path(psidest).and_then(|parent| {
            if psznewname.is_null() {
                return None;
            }
            Some(parent.join(OsString::from_wide(unsafe { psznewname.as_wide() })))
        });
        self.tracker
            .record_hresult(hr, "Create item", source, shell_path(psinewitem));
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

/// Positive HRESULTs can describe decisions, pending work, or skipped items.
/// Only documented completion codes imply a successful item.
pub fn classify_hresult(hr: HRESULT) -> ItemStatus {
    if hr == COPYENGINE_E_USER_CANCELLED
        || hr == COPYENGINE_E_CANCELLED
        || hr.0 as u32 == 0x800704C7
    {
        ItemStatus::Canceled
    } else if hr == S_OK
        || hr == COPYENGINE_S_COLLISIONRESOLVED
        || hr == COPYENGINE_S_KEEP_BOTH
        || hr == COPYENGINE_S_MERGE
    {
        ItemStatus::Succeeded
    } else if hr.is_ok() {
        ItemStatus::Skipped
    } else {
        ItemStatus::Failed
    }
}
pub(crate) fn shell_path(item: Option<&IShellItem>) -> Option<PathBuf> {
    let name = unsafe { item?.GetDisplayName(SIGDN_FILESYSPATH) }.ok()?;
    let path = PathBuf::from(OsString::from_wide(unsafe { name.as_wide() }));
    unsafe {
        CoTaskMemFree(Some(name.0.cast()));
    }
    Some(crate::path::ensure_extended_prefix(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Shell::{COPYENGINE_S_PENDING, COPYENGINE_S_USER_IGNORED};
    #[test]
    fn callbacks_classify_skip_cancel_and_failure() {
        let (sink, tracker) = ShellProgressSink::new(false);
        let sink: IFileOperationProgressSink = sink.into();
        unsafe {
            sink.PostCopyItem(
                0,
                None,
                None,
                PCWSTR::null(),
                COPYENGINE_S_USER_IGNORED,
                None,
            )
            .unwrap();
            sink.PostMoveItem(
                0,
                None,
                None,
                PCWSTR::null(),
                COPYENGINE_E_USER_CANCELLED,
                None,
            )
            .unwrap();
            sink.PostCopyItem(0, None, None, PCWSTR::null(), E_FAIL, None)
                .unwrap();
        }
        let report = tracker.report();
        assert_eq!(report.completed, 0);
        assert_eq!(report.failed, 1);
        assert!(report.was_aborted);
        assert_eq!(report.item_results[0].status, ItemStatus::Skipped);
        assert_eq!(
            report.item_results[0].native_code,
            Some(COPYENGINE_S_USER_IGNORED.0 as u32)
        );
        assert_eq!(report.item_results[1].status, ItemStatus::Canceled);
        assert_eq!(classify_hresult(COPYENGINE_S_PENDING), ItemStatus::Skipped);
        assert_eq!(classify_hresult(S_OK), ItemStatus::Succeeded);
    }
    #[test]
    fn recycle_denial_keeps_source_outcome_failed() {
        let (sink, tracker) = ShellProgressSink::new(true);
        let sink: IFileOperationProgressSink = sink.into();
        assert!(unsafe { sink.PreDeleteItem(0, None) }.is_err());
        assert!(tracker.report().was_aborted);
        assert_eq!(tracker.report().item_results[0].status, ItemStatus::Failed);
    }
}
