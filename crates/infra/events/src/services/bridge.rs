//! Cross-replica event relay over Postgres `LISTEN`/`NOTIFY`.
//!
//! In a multi-replica deployment the in-process broadcasters an
//! [`crate::EventRouter`] fans out to only reach SSE connections held by the
//! current process. [`PostgresEventBridge`] closes that gap: every replica runs
//! one bridge task that `LISTEN`s on [`OUTBOX_CHANNEL`]. When any replica
//! routes an event it appends a row to `event_outbox` and emits a `NOTIFY`
//! carrying that row's id. Each bridge receives the notification, loads the
//! row, deserializes the payload by its `channel`, and re-injects the event
//! through the router's *local-only* path — which deliberately does **not**
//! touch the outbox, so the relay cannot loop.
//!
//! The same listener also carries [`CACHE_INVALIDATION_CHANNEL`], whose
//! payload is a cache domain; see [`super::cache_invalidation`].
//!
//! `LISTEN` holds a Postgres session for the bridge's lifetime, so the
//! listener connects through its own pool
//! ([`PostgresEventBridge::with_listener_pool`]): a direct, session-capable
//! connection when the main pool sits behind a transaction-mode pooler.
//! `NOTIFY` and the outbox writes are ordinary statements on the main pool.
//!
//! The notification payload is only the row id (a UUID string) to stay
//! well under Postgres' ~8 KB `NOTIFY` limit; the event body lives in the
//! `jsonb` column.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use systemprompt_traits::CacheEpochs;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::bridge_handle::{EventBridgeHandle, RelayStatus, StatusCell};
use super::cache_invalidation::{
    CACHE_INVALIDATION_CHANNEL, invalidate_all_cache_domains, receive_cache_invalidation,
};

use super::repository::EventOutboxRepository;
use super::routing::{EventBroadcasters, EventRouter, OUTBOX_CHANNEL, OutboxChannel};
use systemprompt_identifiers::{EventOutboxId, InstanceId, UserId};
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, SystemEvent};

const OUTBOX_RETENTION: Duration = Duration::from_secs(3600);
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);
const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);

// Why: Postgres rejects LISTEN on a standby with SQLSTATE 25006.
const READ_ONLY_SQL_TRANSACTION: &str = "25006";

fn is_read_only_standby(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some(READ_ONLY_SQL_TRANSACTION),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct PostgresEventBridge {
    listener_pool: PgPool,
    outbox: EventOutboxRepository,
    epochs: CacheEpochs,
    broadcasters: Arc<EventBroadcasters>,
}

impl PostgresEventBridge {
    #[must_use]
    pub fn new(
        pool: PgPool,
        instance_id: InstanceId,
        epochs: CacheEpochs,
        broadcasters: Arc<EventBroadcasters>,
    ) -> Self {
        Self {
            outbox: EventOutboxRepository::new(pool.clone(), instance_id),
            listener_pool: pool,
            epochs,
            broadcasters,
        }
    }

    #[must_use]
    pub fn with_listener_pool(mut self, listener_pool: PgPool) -> Self {
        self.listener_pool = listener_pool;
        self
    }

    #[must_use]
    pub fn router(&self) -> EventRouter {
        EventRouter::from_outbox(self.outbox.clone(), Arc::clone(&self.broadcasters))
    }

    #[must_use]
    pub fn into_idle_handle(self) -> EventBridgeHandle {
        EventBridgeHandle::idle(self)
    }

    pub fn start(self) -> EventBridgeHandle {
        let handle = self.into_idle_handle();
        handle.start();
        handle
    }

    async fn open_listener(&self) -> Result<PgListener, sqlx::Error> {
        let mut listener = PgListener::connect_with(&self.listener_pool).await?;
        listener
            .listen_all([OUTBOX_CHANNEL, CACHE_INVALIDATION_CHANNEL])
            .await?;
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

    pub(super) async fn run(self, status: &StatusCell, cancel: CancellationToken) {
        let mut prune_tick = tokio::time::interval(PRUNE_INTERVAL);
        prune_tick.tick().await;
        let mut backoff = RETRY_MIN;

        loop {
            self.epochs.set_relay_live(false);
            status.set(RelayStatus::Reconnecting);
            let mut listener = match self.open_listener().await {
                Ok(listener) => listener,
                Err(e) => {
                    Self::report_listener_failure(&e, backoff);
                    tokio::select! {
                        () = cancel.cancelled() => break,
                        () = tokio::time::sleep(backoff) => {},
                    }
                    backoff = (backoff * 2).min(RETRY_MAX);
                    continue;
                },
            };
            backoff = RETRY_MIN;
            invalidate_all_cache_domains(&self.epochs);
            self.epochs.set_relay_live(true);
            status.set(RelayStatus::Listening);
            info!(
                channel = OUTBOX_CHANNEL,
                "event bridge: listening for cross-replica events"
            );

            let keep_going = self.serve(&mut listener, &mut prune_tick, &cancel).await;
            if !keep_going {
                break;
            }
        }
        self.epochs.set_relay_live(false);
        status.set(RelayStatus::Stopped);
        info!("event bridge: stopped");
    }

    async fn serve(
        &self,
        listener: &mut PgListener,
        prune_tick: &mut tokio::time::Interval,
        cancel: &CancellationToken,
    ) -> bool {
        loop {
            tokio::select! {
                () = cancel.cancelled() => return false,
                notification = listener.recv() => match notification {
                    Ok(notification) if notification.channel() == CACHE_INVALIDATION_CHANNEL => {
                        receive_cache_invalidation(&self.epochs, notification.payload());
                    },
                    Ok(notification) => {
                        self.deliver(&EventOutboxId::new(notification.payload())).await;
                    },
                    Err(e) => {
                        warn!(error = %e, "event bridge: listener connection lost; reconnecting");
                        return true;
                    },
                },
                _ = prune_tick.tick() => {
                    self.prune().await;
                },
            }
        }
    }

    async fn deliver(&self, id: &EventOutboxId) {
        let row = match self.outbox.find(id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                debug!(row_id = %id, "event bridge: outbox row already pruned; skipping");
                return;
            },
            Err(e) => {
                error!(error = %e, row_id = %id, "event bridge: failed to load outbox row");
                return;
            },
        };

        let Some(channel) = OutboxChannel::parse(&row.channel) else {
            error!(channel = %row.channel, row_id = %id, "event bridge: unknown outbox channel");
            return;
        };
        if !row.deliver_to_origin && &row.origin_instance_id == self.outbox.instance_id() {
            debug!(
                row_id = %id,
                "event bridge: own event already routed locally; skipping"
            );
            return;
        }

        self.fan_in(channel, &row.user_id, row.payload).await;
    }

    pub(super) async fn fan_in(
        &self,
        channel: OutboxChannel,
        user_id: &UserId,
        // JSON: outbox payload is polymorphic by channel; decoded into the
        // matching typed event immediately below.
        payload: serde_json::Value,
    ) {
        let local = EventRouter::local_only(Arc::clone(&self.broadcasters));
        match channel {
            OutboxChannel::AgUi => match serde_json::from_value::<AgUiEvent>(payload) {
                Ok(event) => {
                    local.route_agui_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode AG-UI event"),
            },
            OutboxChannel::A2A => match serde_json::from_value::<A2AEvent>(payload) {
                Ok(event) => {
                    local.route_a2a_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode A2A event"),
            },
            OutboxChannel::System => match serde_json::from_value::<SystemEvent>(payload) {
                Ok(event) => {
                    local.route_system_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode system event"),
            },
            OutboxChannel::Analytics => match serde_json::from_value::<AnalyticsEvent>(payload) {
                Ok(event) => {
                    local.route_analytics_local(user_id, event).await;
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
