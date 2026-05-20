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

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::repository::EventOutboxRepository;
use super::routing::{EventRouter, OUTBOX_CHANNEL, OutboxChannel};
use systemprompt_identifiers::UserId;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, SystemEvent};

/// Rows older than this are pruned opportunistically by the bridge.
const OUTBOX_RETENTION: Duration = Duration::from_secs(3600);

/// How often the bridge runs an opportunistic prune sweep.
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);

/// Background relay that mirrors `event_outbox` rows into the local
/// broadcasters of the replica it runs on.
#[derive(Debug, Clone)]
pub struct PostgresEventBridge {
    pool: PgPool,
}

impl PostgresEventBridge {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Installs the relay pool on [`EventRouter`] and spawns the listener
    /// task. The returned handle resolves when the listener stops; abort it
    /// to shut the relay down.
    pub fn start(self) -> JoinHandle<()> {
        EventRouter::install_relay(self.pool.clone());
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(self) {
        let mut prune_tick = tokio::time::interval(PRUNE_INTERVAL);
        prune_tick.tick().await;

        loop {
            let mut listener = match PgListener::connect_with(&self.pool).await {
                Ok(listener) => listener,
                Err(e) => {
                    error!(error = %e, "event bridge: failed to open Postgres listener; retrying");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                },
            };
            if let Err(e) = listener.listen(OUTBOX_CHANNEL).await {
                error!(error = %e, channel = OUTBOX_CHANNEL, "event bridge: LISTEN failed; retrying");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
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

    /// Loads the outbox row named by `row_id`, deserializes it, and fans it
    /// into the local broadcasters. This is the relay's fan-in entry point.
    async fn deliver(&self, row_id: &str) {
        let repo = EventOutboxRepository::new(self.pool.clone());
        let row = match repo.find(row_id).await {
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
        let user_id = UserId::new(row.user_id);
        Self::fan_in(channel, &user_id, row.payload).await;
    }

    /// Deserializes `payload` according to `channel` and routes it through
    /// the local-only path so it never re-enters the outbox.
    ///
    /// Exposed within the crate so the relay logic can be exercised
    /// directly without a live Postgres listener.
    pub(crate) async fn fan_in(
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
        let repo = EventOutboxRepository::new(self.pool.clone());
        match repo.prune(cutoff).await {
            Ok(deleted) => {
                if deleted > 0 {
                    debug!(deleted, "event bridge: pruned expired outbox rows");
                }
            },
            Err(e) => error!(error = %e, "event bridge: outbox prune failed"),
        }
    }
}
