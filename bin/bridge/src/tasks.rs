//! Owned background work with inspected completion and visible panic reports.
//!
//! `TaskOwner` supervises async work on the bridge runtime; `OwnedThread`
//! owns one blocking OS thread that is told to stop, woken and joined.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::activity::ActivityLog;
use std::future::Future;
use std::pin::Pin;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::{JoinHandle, JoinSet};

type Work = (
    &'static std::panic::Location<'static>,
    Pin<Box<dyn Future<Output = ()> + Send>>,
);

pub(crate) struct TaskOwner {
    sender: mpsc::UnboundedSender<Work>,
    supervisor: JoinHandle<()>,
    activity: ActivityLog,
}

impl TaskOwner {
    pub(crate) fn new(runtime: &Handle, activity: ActivityLog) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Work>();
        let log = activity.clone();
        let supervisor = runtime.spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! {
                    work = receiver.recv() => match work {
                        Some((origin, future)) => {
                            tasks.spawn(async move {
                                use futures_util::FutureExt;
                                (origin, std::panic::AssertUnwindSafe(future).catch_unwind().await)
                            });
                        },
                        None => break,
                    },
                    result = tasks.join_next(), if !tasks.is_empty() => match result {
                        Some(Ok((origin, Err(_)))) => log.append_error(format!("background task at {origin} panicked; operation did not complete")),
                        Some(Err(e)) => log.append_error(format!("background task failed: {e}")),
                        Some(Ok((_, Ok(())))) | None => {},
                    },
                }
            }
            tasks.shutdown().await;
        });
        Self {
            sender,
            supervisor,
            activity,
        }
    }

    #[track_caller]
    pub(crate) fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        if self
            .sender
            .send((std::panic::Location::caller(), Box::pin(future)))
            .is_err()
        {
            self.activity.append_error(format!(
                "background task at {} was not started: task supervisor stopped",
                std::panic::Location::caller()
            ));
        }
    }
}

impl Drop for TaskOwner {
    fn drop(&mut self) {
        self.supervisor.abort();
    }
}

impl std::fmt::Debug for TaskOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskOwner")
            .field("stopped", &self.supervisor.is_finished())
            .finish_non_exhaustive()
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) use owned_thread::{OwnedThread, OwnedThreadError, StopSignal};

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod owned_thread {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug, Clone)]
    pub(crate) struct StopSignal(Arc<AtomicBool>);

    impl StopSignal {
        pub(crate) fn is_raised(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum OwnedThreadError {
        #[error("thread {name} could not be woken to stop; it was left running: {source}")]
        Wake {
            name: &'static str,
            #[source]
            source: std::io::Error,
        },
        #[error("thread {name} panicked before it stopped")]
        Panicked { name: &'static str },
    }

    type Wake = Box<dyn Fn() -> std::io::Result<()> + Send>;

    pub(crate) struct OwnedThread {
        name: &'static str,
        stop: StopSignal,
        wake: Wake,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl OwnedThread {
        pub(crate) fn spawn(
            name: &'static str,
            body: impl FnOnce(StopSignal) + Send + 'static,
            wake: impl Fn() -> std::io::Result<()> + Send + 'static,
        ) -> std::io::Result<Self> {
            let stop = StopSignal(Arc::new(AtomicBool::new(false)));
            let signal = stop.clone();
            let handle = std::thread::Builder::new()
                .name(name.to_owned())
                .spawn(move || body(signal))?;
            Ok(Self {
                name,
                stop,
                wake: Box::new(wake),
                handle: Some(handle),
            })
        }

        pub(crate) fn stop(mut self) -> Result<(), OwnedThreadError> {
            self.stop_and_join()
        }

        fn stop_and_join(&mut self) -> Result<(), OwnedThreadError> {
            let Some(handle) = self.handle.take() else {
                return Ok(());
            };
            self.stop.0.store(true, Ordering::Release);
            if !handle.is_finished()
                && let Err(source) = (self.wake)()
            {
                return Err(OwnedThreadError::Wake {
                    name: self.name,
                    source,
                });
            }
            handle
                .join()
                .map_err(|_panic| OwnedThreadError::Panicked { name: self.name })
        }
    }

    impl Drop for OwnedThread {
        fn drop(&mut self) {
            if let Err(e) = self.stop_and_join() {
                tracing::error!(error = %e, "owned thread did not stop cleanly");
            }
        }
    }

    impl std::fmt::Debug for OwnedThread {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("OwnedThread")
                .field("name", &self.name)
                .field(
                    "running",
                    &self.handle.as_ref().is_some_and(|h| !h.is_finished()),
                )
                .finish_non_exhaustive()
        }
    }
}
