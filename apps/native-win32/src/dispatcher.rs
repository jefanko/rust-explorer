#![allow(dead_code)]

use explorer_app::requests::CompletionEnvelope;
use explorer_domain::errors::ExplorerError;
use explorer_domain::models::{DirectoryPage, NavigationResponse, PreviewData};
use explorer_domain::operations::JobSummary;
use explorer_index::SearchResponse;
use explorer_watch::events::WatchNotification;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub const WM_APP_WAKEUP: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;

/// Bounded events dispatched from background tasks to the UI thread.
#[derive(Debug)]
pub enum UiEvent {
    DirectoryPageReady(CompletionEnvelope<DirectoryPage>),
    NavigationReady(CompletionEnvelope<NavigationResponse>),
    RefreshReady(CompletionEnvelope<NavigationResponse>),
    WatchNotification(WatchNotification),
    SearchReady(CompletionEnvelope<SearchResponse>),
    PreviewReady(CompletionEnvelope<PreviewData>),
    JobCompleted(CompletionEnvelope<JobSummary>),
    SearchIndexFailed(ExplorerError),
    StatusMessage(String),
    OperationError(ExplorerError),
}

pub struct Dispatcher {
    hwnd: AtomicUsize,
    queue: Mutex<VecDeque<UiEvent>>,
    has_pending_wakeup: AtomicBool,
    capacity: usize,
}

unsafe impl Send for Dispatcher {}
unsafe impl Sync for Dispatcher {}

impl Dispatcher {
    pub fn new(hwnd: HWND, capacity: usize) -> Self {
        Self {
            hwnd: AtomicUsize::new(hwnd.0 as usize),
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            has_pending_wakeup: AtomicBool::new(false),
            capacity,
        }
    }

    /// Binds the dispatcher to the real top-level window.
    ///
    /// The window is created hidden and only published afterwards so no
    /// completion can be enqueued against a null HWND and then stranded.
    pub fn set_hwnd(&self, hwnd: HWND) {
        self.hwnd.store(hwnd.0 as usize, Ordering::SeqCst);
        if self.has_pending_wakeup.swap(true, Ordering::SeqCst) {
            return;
        }
        if !self.post_wakeup() {
            self.has_pending_wakeup.store(false, Ordering::SeqCst);
        }
    }

    fn post_wakeup(&self) -> bool {
        let raw = self.hwnd.load(Ordering::SeqCst);
        if raw == 0 {
            return false;
        }
        unsafe { PostMessageW(HWND(raw as *mut _), WM_APP_WAKEUP, WPARAM(0), LPARAM(0)) }.is_ok()
    }

    /// Enqueues an event from a background worker and wakes up the UI message loop.
    pub fn enqueue(&self, event: UiEvent) {
        {
            let mut q = self.queue.lock().unwrap();
            if q.len() >= self.capacity {
                tracing::warn!(
                    "UI dispatcher queue saturated (capacity: {}); shedding older event",
                    self.capacity
                );
                q.pop_front();
            }
            q.push_back(event);
        }

        // Coalesce wakeup messages: only post if no pending wakeup is already in flight.
        if !self.has_pending_wakeup.swap(true, Ordering::SeqCst) && !self.post_wakeup() {
            self.has_pending_wakeup.store(false, Ordering::SeqCst);
        }
    }

    /// Drains up to `limit` events on the UI thread without starving input/paint.
    pub fn drain_batch(&self, limit: usize) -> Vec<UiEvent> {
        let (events, remaining) = {
            let mut q = self.queue.lock().unwrap();
            let count = q.len().min(limit);
            let drained: Vec<UiEvent> = q.drain(..count).collect();
            let rem = q.len();
            (drained, rem)
        };

        // Reset the wakeup flag
        self.has_pending_wakeup.store(false, Ordering::SeqCst);

        // If items remain in queue, repost wakeup immediately to prevent stranding events
        if remaining > 0
            && !self.has_pending_wakeup.swap(true, Ordering::SeqCst)
            && !self.post_wakeup()
        {
            self.has_pending_wakeup.store(false, Ordering::SeqCst);
        }

        events
    }
}

pub type SharedDispatcher = Arc<Dispatcher>;

#[cfg(test)]
mod tests {
    use super::*;
    use explorer_domain::errors::ErrorCode;

    #[test]
    fn test_dispatcher_capacity_bounds() {
        let dispatcher = Dispatcher::new(HWND(std::ptr::null_mut()), 3);
        dispatcher.enqueue(UiEvent::OperationError(ExplorerError::new(
            ErrorCode::NotFound,
            "1",
            "test",
        )));
        dispatcher.enqueue(UiEvent::OperationError(ExplorerError::new(
            ErrorCode::NotFound,
            "2",
            "test",
        )));
        dispatcher.enqueue(UiEvent::OperationError(ExplorerError::new(
            ErrorCode::NotFound,
            "3",
            "test",
        )));
        // 4th event should evict the 1st
        dispatcher.enqueue(UiEvent::OperationError(ExplorerError::new(
            ErrorCode::NotFound,
            "4",
            "test",
        )));

        let drained = dispatcher.drain_batch(10);
        assert_eq!(drained.len(), 3);
        match &drained[0] {
            UiEvent::OperationError(err) => assert_eq!(err.user_message, "2"),
            _ => panic!("unexpected event"),
        }
        match &drained[2] {
            UiEvent::OperationError(err) => assert_eq!(err.user_message, "4"),
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn test_dispatcher_drain_batch_limit() {
        let dispatcher = Dispatcher::new(HWND(std::ptr::null_mut()), 10);
        for i in 0..5 {
            dispatcher.enqueue(UiEvent::OperationError(ExplorerError::new(
                ErrorCode::NotFound,
                i.to_string(),
                "test",
            )));
        }

        let first_batch = dispatcher.drain_batch(2);
        assert_eq!(first_batch.len(), 2);

        let second_batch = dispatcher.drain_batch(2);
        assert_eq!(second_batch.len(), 2);

        let final_batch = dispatcher.drain_batch(2);
        assert_eq!(final_batch.len(), 1);

        let empty_batch = dispatcher.drain_batch(2);
        assert_eq!(empty_batch.len(), 0);
    }
}
