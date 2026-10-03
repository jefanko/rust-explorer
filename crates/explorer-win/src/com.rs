use explorer_domain::errors::{ErrorCode, ExplorerError};
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, JoinHandle};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Dedicated Single-Threaded Apartment (STA) worker thread for COM operations.
pub struct StaWorker {
    sender: Sender<Task>,
    _handle: Option<JoinHandle<()>>,
}

impl StaWorker {
    pub fn new(name: &str) -> Result<Self, ExplorerError> {
        let (tx, rx) = channel::<Task>();

        let thread_name = name.to_string();
        let handle = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                unsafe {
                    let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                    if hr.is_err() {
                        tracing::error!("Failed to initialize COM STA on thread {}", thread_name);
                        return;
                    }
                }

                while let Ok(task) = rx.recv() {
                    task();
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

        Ok(Self {
            sender: tx,
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
            .send(Box::new(move || {
                let res = f();
                let _ = res_tx.send(res);
            }))
            .map_err(|_| {
                ExplorerError::new(
                    ErrorCode::Internal,
                    "STA worker thread channel disconnected",
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
