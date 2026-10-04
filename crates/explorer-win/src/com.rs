use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError, channel, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Dedicated Single-Threaded Apartment (STA) worker thread for COM operations.
pub struct StaWorker {
    sender: Option<SyncSender<Task>>,
    _handle: Option<JoinHandle<()>>,
}

impl StaWorker {
    pub fn new(name: &str) -> Result<Self, ExplorerError> {
        let (tx, rx) = sync_channel::<Task>(64);
        let (ready_tx, ready_rx) = channel();

        let thread_name = name.to_string();
        let handle = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                unsafe {
                    let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                    if hr.is_err() {
                        tracing::error!("Failed to initialize COM STA on thread {}", thread_name);
                        let _ = ready_tx.send(Err(hr.0 as u32));
                        return;
                    }
                }
                let _ = ready_tx.send(Ok(()));

                loop {
                    pump_messages();
                    match rx.recv_timeout(Duration::from_millis(20)) {
                        Ok(task) => {
                            pump_messages();
                            task();
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }

                unsafe {
                    CoUninitialize();
                }
            })
            .map_err(|e| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    format!("Failed to spawn STA thread: {e}"),
                    "StaWorker::new",
                )
            })?;

        if ready_rx.recv().unwrap_or(Err(0)).is_err() {
            let _ = handle.join();
            return Err(ExplorerError::new(
                ErrorCode::Internal,
                "Dedicated STA COM initialization failed",
                "StaWorker::new",
            ));
        }
        Ok(Self {
            sender: Some(tx),
            _handle: Some(handle),
        })
    }

    /// Dispatches a task to the STA thread and returns a receiver for the result.
    pub fn execute<F, R>(&self, f: F) -> Result<R, ExplorerError>
    where
        F: FnOnce() -> Result<R, ExplorerError> + Send + 'static,
        R: Send + 'static,
    {
        let (res_tx, res_rx) = channel();

        self.sender
            .as_ref()
            .ok_or_else(|| {
                ExplorerError::new(
                    ErrorCode::Canceled,
                    "STA is shutting down",
                    "StaWorker::execute",
                )
            })?
            .try_send(Box::new(move || {
                let res = f();
                let _ = res_tx.send(res);
            }))
            .map_err(|error| {
                ExplorerError::new(
                    if matches!(error, TrySendError::Full(_)) {
                        ErrorCode::QueueFull
                    } else {
                        ErrorCode::Internal
                    },
                    "STA worker queue is full or disconnected",
                    "StaWorker::execute",
                )
            })?;

        res_rx.recv().map_err(|_| {
            ExplorerError::new(
                ErrorCode::Internal,
                "STA worker thread dropped response",
                "StaWorker::execute",
            )
        })?
    }
}

fn pump_messages() {
    let mut message = MSG::default();
    // Bound each drain so a message flood cannot starve the job queue.
    for _ in 0..128 {
        unsafe {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
impl Drop for StaWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(handle) = self._handle.take() {
            let _ = handle.join();
        }
    }
}
