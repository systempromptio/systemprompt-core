//! Readiness flag and readiness-event broadcast owned by the running server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;

const READINESS_CHANNEL_CAPACITY: usize = 16;

#[derive(Debug, Clone, Copy)]
pub enum ReadinessEvent {
    ApiReady,
    ApiShuttingDown,
}

/// The admission signal of one API server.
///
/// A flag the probes read and a broadcast every drain-aware task subscribes
/// to. Built once in `bind_and_serve` and shared by handle; a second server in
/// the same process (tests) has its own.
#[derive(Debug)]
pub struct ReadinessState {
    ready: AtomicBool,
    sender: broadcast::Sender<ReadinessEvent>,
}

impl Default for ReadinessState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadinessState {
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(READINESS_CHANNEL_CAPACITY);
        Self {
            ready: AtomicBool::new(false),
            sender,
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ReadinessEvent> {
        self.sender.subscribe()
    }

    pub fn signal_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
        if self.sender.send(ReadinessEvent::ApiReady).is_err() {
            tracing::debug!("No readiness receivers subscribed");
        }
    }

    pub fn signal_shutdown(&self) {
        self.ready.store(false, Ordering::SeqCst);
        if self.sender.send(ReadinessEvent::ApiShuttingDown).is_err() {
            tracing::debug!("No readiness receivers subscribed");
        }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    pub async fn wait_for_ready(&self, timeout_secs: u64) -> bool {
        let mut receiver = self.subscribe();
        if self.is_ready() {
            return true;
        }

        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
            while let Ok(event) = receiver.recv().await {
                if matches!(event, ReadinessEvent::ApiReady) {
                    return true;
                }
            }
            false
        })
        .await
        .map_err(|_e| {
            tracing::debug!(timeout_secs = timeout_secs, "Readiness wait timed out");
        })
        .unwrap_or(false)
    }
}
