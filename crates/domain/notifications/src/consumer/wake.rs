//! The consumer's wake-up: a `LISTEN` on the notification facts channel on
//! the listener pool, with the poll interval as a backstop. A lost or
//! unavailable listener is retried with exponential backoff while the
//! consumer keeps polling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use systemprompt_events::NOTIFICATION_FACTS_CHANNEL;
use tokio_util::sync::CancellationToken;

use super::{ConsumerState, StateCell};

const BACKOFF_START: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

pub(super) struct Wake {
    pool: PgPool,
    state: StateCell,
    listener: Option<PgListener>,
    backoff: Duration,
    retry_at: Instant,
}

impl Wake {
    pub(super) fn new(pool: PgPool, state: StateCell) -> Self {
        Self {
            pool,
            state,
            listener: None,
            backoff: BACKOFF_START,
            retry_at: Instant::now(),
        }
    }

    async fn connect(&mut self) {
        if self.listener.is_some() || Instant::now() < self.retry_at {
            return;
        }
        match listen(&self.pool).await {
            Ok(listener) => {
                self.listener = Some(listener);
                self.backoff = BACKOFF_START;
            },
            Err(error) => {
                tracing::warn!(error = %error, retry_in_secs = self.backoff.as_secs(), "Notification listener unavailable; polling");
                self.lost();
            },
        }
    }

    fn lost(&mut self) {
        self.listener = None;
        self.retry_at = Instant::now() + self.backoff;
        self.backoff = (self.backoff * 2).min(BACKOFF_MAX);
    }

    pub(super) async fn wait(&mut self, poll: Duration, shutdown: &CancellationToken) -> bool {
        self.connect().await;
        let Some(listener) = self.listener.as_mut() else {
            self.state.set(ConsumerState::Reconnecting);
            return tokio::select! {
                () = shutdown.cancelled() => false,
                () = tokio::time::sleep(poll) => true,
            };
        };
        self.state.set(ConsumerState::Listening);
        let lost = tokio::select! {
            () = shutdown.cancelled() => return false,
            () = tokio::time::sleep(poll) => None,
            received = listener.recv() => received.err(),
        };
        if let Some(error) = lost {
            tracing::warn!(error = %error, "Notification listener lost; reconnecting");
            self.lost();
        }
        true
    }
}

async fn listen(pool: &PgPool) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(NOTIFICATION_FACTS_CHANNEL).await?;
    Ok(listener)
}
