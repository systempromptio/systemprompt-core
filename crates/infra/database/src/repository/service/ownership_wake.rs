//! The wake-up behind process hand-over: a worker that withdraws its presence
//! or releases a process lease notifies [`PROCESS_OWNERSHIP_CHANNEL`], and
//! every worker's lease loop waits on a [`ProcessOwnershipWake`] so it
//! re-evaluates at once instead of at its next tick.
//!
//! The notification carries no state: a woken worker reads the reachable set
//! and claims as it would on a tick, so a lost notification costs only the
//! wait until that tick. The listener needs a session-level `LISTEN`, the
//! same constraint as the cache-invalidation channel, and is retried with
//! exponential backoff; without it [`ProcessOwnershipWake::wait`] is a plain
//! timeout. A listener that has just (re)connected reports a wake, because
//! notifications sent while it was away are gone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::PgPool;
use sqlx::postgres::PgListener;

use crate::DbPool;

pub const PROCESS_OWNERSHIP_CHANNEL: &str = "systemprompt_process_ownership";

const BACKOFF_START: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// A listener on [`PROCESS_OWNERSHIP_CHANNEL`] with a timeout as its backstop.
#[derive(Debug)]
pub struct ProcessOwnershipWake {
    pool: Arc<PgPool>,
    listener: Option<PgListener>,
    backoff: Duration,
    retry_at: Instant,
}

impl ProcessOwnershipWake {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pool: db.session_pool(),
            listener: None,
            backoff: BACKOFF_START,
            retry_at: Instant::now(),
        }
    }

    pub const fn is_listening(&self) -> bool {
        self.listener.is_some()
    }

    async fn connect(&mut self) -> bool {
        if self.listener.is_some() || Instant::now() < self.retry_at {
            return false;
        }
        match listen(&self.pool).await {
            Ok(listener) => {
                self.listener = Some(listener);
                self.backoff = BACKOFF_START;
                true
            },
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    retry_in_secs = self.backoff.as_secs(),
                    "process ownership listener unavailable; hand-overs wait for the lease tick"
                );
                self.lost();
                false
            },
        }
    }

    fn lost(&mut self) {
        self.listener = None;
        self.retry_at = Instant::now() + self.backoff;
        self.backoff = (self.backoff * 2).min(BACKOFF_MAX);
    }

    pub async fn wait(&mut self, timeout: Duration) -> bool {
        if self.connect().await {
            return true;
        }
        let Some(listener) = self.listener.as_mut() else {
            tokio::time::sleep(timeout).await;
            return false;
        };
        let received = tokio::select! {
            () = tokio::time::sleep(timeout) => return false,
            received = listener.recv() => received,
        };
        match received {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!(error = %error, "process ownership listener lost; reconnecting");
                self.lost();
                false
            },
        }
    }
}

async fn listen(pool: &PgPool) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(PROCESS_OWNERSHIP_CHANNEL).await?;
    Ok(listener)
}
