//! Owned background work with inspected completion and visible panic reports.
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
