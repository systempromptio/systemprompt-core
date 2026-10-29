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
//! Fact-only rows notify [`crate::NOTIFICATION_FACTS_CHANNEL`] instead, which
//! the bridge does not listen on, so the relay never wakes for a fact.
//!
//! The listener never reconnects on its own: `sqlx` would otherwise
//! re-establish a dropped session inside `recv` and hide the gap, so a closed
//! session ends the serving loop and every reconnect goes through the
//! replay. A reconnecting listener replays the rows it missed, or tells every
//! resumable topic the relay lost events; see [`super::bridge_replay`].
//!
//! The same listener also carries [`CACHE_INVALIDATION_CHANNEL`], whose
//! payload is a cache domain; see [`super::cache_invalidation`].
//!
//! `LISTEN` holds a Postgres session for the bridge's lifetime, so the
//! listener connects through its own pool
//! ([`PostgresEventBridge::with_listener_pool`]): a direct, session-capable
//! connection behind a transaction-mode pooler. `NOTIFY` and the outbox writes
//! are ordinary statements on the main pool.
//!
//! The payload is only the row id, well under the ~8 KB `NOTIFY` limit.
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
use super::bridge_replay::ReplayCursor;
use super::cache_invalidation::{
    CACHE_INVALIDATION_CHANNEL, invalidate_all_cache_domains, receive_cache_invalidation,
};

use super::repository::{EventOutboxRepository, OutboxRow};
use super::routing::{EventBroadcasters, EventRouter, OUTBOX_CHANNEL, OutboxChannel};
use systemprompt_identifiers::{EventOutboxId, InstanceId};

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
    pub const fn listener_pool(&self) -> &PgPool {
        &self.listener_pool
    }

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

    pub(super) const fn outbox(&self) -> &EventOutboxRepository {
        &self.outbox
    }

    pub(super) const fn broadcasters(&self) -> &Arc<EventBroadcasters> {
        &self.broadcasters
    }

    async fn open_listener(&self) -> Result<PgListener, sqlx::Error> {
        let mut listener = PgListener::connect_with(&self.listener_pool).await?;
        listener.eager_reconnect(false);
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
        let mut cursor = ReplayCursor::default();

        loop {
            self.epochs.set_relay_live(false);
            status.set(RelayStatus::Reconnecting);
            let mut listener = match self.open_listener().await {
                Ok(listener) => listener,
                Err(e) => {
                    Self::report_listener_failure(&e, backoff);
                    tokio::select! {
                        () = cancel.cancelled() => break,
                        () = tokio::time::sleep(systemprompt_traits::backoff::jittered(backoff)) => {},
                    }
                    backoff = (backoff * 2).min(RETRY_MAX);
                    continue;
                },
            };
            backoff = RETRY_MIN;
            invalidate_all_cache_domains(&self.epochs);
            self.recover(&mut cursor, OUTBOX_RETENTION).await;
            self.epochs.set_relay_live(true);
            status.set(RelayStatus::Listening);
            info!(
                channel = OUTBOX_CHANNEL,
                "event bridge: listening for cross-replica events"
            );

            let keep_going = self
                .serve(&mut listener, &mut prune_tick, &mut cursor, &cancel)
                .await;
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
        cursor: &mut ReplayCursor,
        cancel: &CancellationToken,
    ) -> bool {
        loop {
            tokio::select! {
                () = cancel.cancelled() => return false,
                notification = listener.try_recv() => match notification {
                    Ok(Some(notification)) if notification.channel() == CACHE_INVALIDATION_CHANNEL => {
                        receive_cache_invalidation(&self.epochs, notification.payload());
                    },
                    Ok(Some(notification)) => {
                        self.deliver(&EventOutboxId::new(notification.payload()), cursor).await;
                    },
                    Ok(None) => {
                        warn!("event bridge: listener connection closed; reconnecting");
                        return true;
                    },
                    Err(e) => {
                        warn!(error = %e, "event bridge: listener connection lost; reconnecting");
                        return true;
                    },
                },
                _ = prune_tick.tick() => {
                    self.prune().await;
                    self.mark_current(cursor).await;
                },
            }
        }
    }

    async fn deliver(&self, id: &EventOutboxId, cursor: &mut ReplayCursor) {
        let row = match self.outbox.find(id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                debug!(row_id = %id, "event bridge: outbox row already pruned; skipping");
                return;
            },
            Err(e) => {
                error!(error = %e, row_id = %id, "event bridge: failed to load outbox row");
                self.relay_lost("outbox row unreadable");
                return;
            },
        };
        if cursor.first_sighting(&row) {
            self.relay_row(row).await;
        }
    }

    pub(super) async fn relay_row(&self, row: OutboxRow) {
        let Some(channel) = OutboxChannel::parse(&row.channel) else {
            error!(channel = %row.channel, row_id = %row.id, "event bridge: unknown outbox channel");
            return;
        };
        if channel == OutboxChannel::Fact {
            debug!(row_id = %row.id, "event bridge: fact-only outbox row carries no live frame");
            return;
        }
        if !row.deliver_to_origin && &row.origin_instance_id == self.outbox.instance_id() {
            debug!(
                row_id = %row.id,
                "event bridge: own event already routed locally; skipping"
            );
            return;
        }
        self.fan_in(channel, &row.user_id, row.payload).await;
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
