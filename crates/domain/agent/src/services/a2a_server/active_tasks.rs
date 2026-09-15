//! Registry of the message pipelines currently running on this server, keyed
//! by task id, so `CancelTask` can reach the worker that owns a task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use systemprompt_identifiers::TaskId;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

#[derive(Debug, Clone, Default)]
pub struct ActiveTasks {
    tokens: Arc<Mutex<HashMap<TaskId, CancellationToken>>>,
    tracker: TaskTracker,
}

/// Removes the task's token from the registry when the pipeline that
/// registered it finishes, however it finishes.
#[derive(Debug)]
pub struct ActiveTaskGuard {
    registry: ActiveTasks,
    task_id: TaskId,
    token: CancellationToken,
}

impl ActiveTaskGuard {
    #[must_use]
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }
}

impl Drop for ActiveTaskGuard {
    fn drop(&mut self) {
        self.registry.remove(&self.task_id);
    }
}

impl ActiveTasks {
    #[must_use]
    pub fn register(&self, task_id: TaskId) -> ActiveTaskGuard {
        let token = CancellationToken::new();
        self.lock().insert(task_id.clone(), token.clone());
        ActiveTaskGuard {
            registry: self.clone(),
            task_id,
            token,
        }
    }

    pub fn cancel(&self, task_id: &TaskId) -> bool {
        self.lock().get(task_id).is_some_and(|token| {
            token.cancel();
            true
        })
    }

    #[must_use]
    pub fn is_running(&self, task_id: &TaskId) -> bool {
        self.lock().contains_key(task_id)
    }

    pub async fn wait_until_finished(
        &self,
        task_id: &TaskId,
        timeout: std::time::Duration,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while self.is_running(task_id) {
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        true
    }

    #[must_use]
    pub const fn tracker(&self) -> &TaskTracker {
        &self.tracker
    }

    fn remove(&self, task_id: &TaskId) {
        self.lock().remove(task_id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<TaskId, CancellationToken>> {
        // Why: the map only holds tokens; a poisoned lock means a panic while
        // inserting or removing one, and the map is still consistent.
        self.tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
