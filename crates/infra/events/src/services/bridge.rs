//! Cross-replica event relay over Postgres `LISTEN`/`NOTIFY`.
//!
//! In a multi-replica deployment the in-process [`crate::EventRouter`]
//! broadcasters only reach SSE connections held by the current process.
//! [`PostgresEventBridge`] closes that gap: every replica runs one bridge
//! task that `LISTEN`s on [`OUTBOX_CHANNEL`]. When any replica routes an
//! event it appends a row to `event_outbox` and emits a `NOTIFY` carrying
//! that row's id. Each bridge receives the notification, loads the row,
//! deserializes the payload by its `channel`, and re-injects the event
//! through the router's *local-only* path — which deliberately does **not**
//! touch the outbox, so the relay cannot loop.
//!
//! The notification payload is only the row id (a UUID string) to stay
//! well under Postgres' ~8 KB `NOTIFY` limit; the event body lives in the
//! `jsonb` column.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::repository::EventOutboxRepository;
use super::routing::{EventRouter, OUTBOX_CHANNEL, OutboxChannel};
use systemprompt_identifiers::{EventOutboxId, UserId};
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, SystemEvent};

const OUTBOX_RETENTION: Duration = Duration::from_secs(3600);
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);
const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);

// Why: Postgres `read_only_sql_transaction`, raised by `LISTEN` on a standby.
const READ_ONLY_SQL_TRANSACTION: &str = "25006";

static LISTENING: AtomicBool = AtomicBool::new(true);

/// `false` once the relay has failed to establish a listener and has not
/// recovered. Stays `true` in deployments that never start a bridge.
#[must_use]
pub fn is_listening() -> bool {
    LISTENING.load(Ordering::Relaxed)
}

fn is_read_only_standby(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some(READ_ONLY_SQL_TRANSACTION),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct PostgresEventBridge {
    pool: PgPool,
    outbox: EventOutboxRepository,
}

impl PostgresEventBridge {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            outbox: EventOutboxRepository::new(pool.clone()),
            pool,
        }
    }

    /// Abort the returned handle to stop the relay.
    pub fn start(self) -> JoinHandle<()> {
        EventRouter::install_relay(self.pool.clone());
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn open_listener(&self) -> Result<PgListener, sqlx::Error> {
        let mut listener = PgListener::connect_with(&self.pool).await?;
        listener.listen(OUTBOX_CHANNEL).await?;
        Ok(listener)
    }

    fn report_listener_failure(err: &sqlx::Error, retry_in: Duration) {
        let retry_in_secs = retry_in.as_secs();
        if is_read_only_standby(err) {
            error!(
                error = %err,
                channel = OUTBOX_CHANNEL,
                retry_in_secs,
                "event bridge: pool is a read-only standby; LISTEN/NOTIFY requires the primary. \
                 Point the write pool (`database_write_url` secret, or `DATABASE_WRITE_URL` with \
                 the env secrets source) at the primary and restart"
            );
        } else {
            error!(
                error = %err,
                channel = OUTBOX_CHANNEL,
                retry_in_secs,
                "event bridge: failed to open Postgres listener; retrying"
            );
        }
    }

    async fn run(self) {
        let mut prune_tick = tokio::time::interval(PRUNE_INTERVAL);
        prune_tick.tick().await;
        let mut backoff = RETRY_MIN;

        loop {
            let mut listener = match self.open_listener().await {
                Ok(listener) => listener,
                Err(e) => {
                    LISTENING.store(false, Ordering::Relaxed);
                    Self::report_listener_failure(&e, backoff);
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(RETRY_MAX);
                    continue;
                },
            };
            backoff = RETRY_MIN;
            LISTENING.store(true, Ordering::Relaxed);
            info!(
                channel = OUTBOX_CHANNEL,
                "event bridge: listening for cross-replica events"
            );

            loop {
                tokio::select! {
                    notification = listener.recv() => match notification {
                        Ok(notification) => {
                            self.deliver(notification.payload()).await;
                        },
                        Err(e) => {
                            warn!(error = %e, "event bridge: listener connection lost; reconnecting");
                            break;
                        },
                    },
                    _ = prune_tick.tick() => {
                        self.prune().await;
                    },
                }
            }
        }
    }

    async fn deliver(&self, row_id: &str) {
        let id = EventOutboxId::new(row_id);
        let row = match self.outbox.find(&id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                debug!(row_id, "event bridge: outbox row already pruned; skipping");
                return;
            },
            Err(e) => {
                error!(error = %e, row_id, "event bridge: failed to load outbox row");
                return;
            },
        };

        let Some(channel) = OutboxChannel::parse(&row.channel) else {
            error!(channel = %row.channel, row_id, "event bridge: unknown outbox channel");
            return;
        };
        Self::fan_in(channel, &row.user_id, row.payload).await;
    }

    pub(super) async fn fan_in(
        channel: OutboxChannel,
        user_id: &UserId,
        // JSON: outbox payload is polymorphic by channel; decoded into the
        // matching typed event immediately below.
        payload: serde_json::Value,
    ) {
        match channel {
            OutboxChannel::AgUi => match serde_json::from_value::<AgUiEvent>(payload) {
                Ok(event) => {
                    EventRouter::route_agui_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode AG-UI event"),
            },
            OutboxChannel::A2A => match serde_json::from_value::<A2AEvent>(payload) {
                Ok(event) => {
                    EventRouter::route_a2a_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode A2A event"),
            },
            OutboxChannel::System => match serde_json::from_value::<SystemEvent>(payload) {
                Ok(event) => {
                    EventRouter::route_system_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode system event"),
            },
            OutboxChannel::Analytics => match serde_json::from_value::<AnalyticsEvent>(payload) {
                Ok(event) => {
                    EventRouter::route_analytics_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode analytics event"),
            },
        }
    }

    async fn prune(&self) {
        let cutoff = chrono::Utc::now()
            - chrono::Duration::from_std(OUTBOX_RETENTION)
                .unwrap_or_else(|_| chrono::Duration::seconds(3600));
        match self.outbox.prune(cutoff).await {
            Ok(deleted) => {
                if deleted > 0 {
                    debug!(deleted, "event bridge: pruned expired outbox rows");
                }
            },
            Err(e) => error!(error = %e, "event bridge: outbox prune failed"),
        }
    }
}
