//! The wake-up behind process hand-over: a worker that withdraws its presence
//! or releases a process lease notifies [`PROCESS_OWNERSHIP_CHANNEL`], and
//! every worker's lease loop waits on a [`ProcessOwnershipWake`] so it
//! re-evaluates at once instead of at its next tick.
//!
//! The notification carries no state: a woken worker reads the reachable set
//! and claims as it would on a tick, so a lost notification costs only the
//! wait until that tick. The channel is one of the node's
//! [`crate::NotificationHub`] channels; without a listening hub
//! [`ProcessOwnershipWake::wait`] is a plain timeout. A hub session that has
//! just (re)connected reports a wake, because notifications sent while it
//! was away are gone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::DbPool;
use crate::services::{PollWake, WakeReason};

pub const PROCESS_OWNERSHIP_CHANNEL: &str = "systemprompt_process_ownership";

/// A subscription to [`PROCESS_OWNERSHIP_CHANNEL`] with a timeout as its
/// backstop.
#[derive(Debug)]
pub struct ProcessOwnershipWake {
    wake: PollWake,
    never: CancellationToken,
}

impl ProcessOwnershipWake {
    pub fn new(db: &DbPool) -> Self {
        Self {
            wake: PollWake::new(db.notifications(), &[PROCESS_OWNERSHIP_CHANNEL]),
            never: CancellationToken::new(),
        }
    }

    pub fn is_listening(&self) -> bool {
        self.wake.is_listening()
    }

    pub async fn wait(&mut self, timeout: Duration) -> bool {
        self.wake.wait(timeout, &self.never).await == WakeReason::Notified
    }
}
