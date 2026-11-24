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
//! The bridge subscribes to the node's one `LISTEN` session, the
//! [`NotificationHub`] on the session pool (a direct, session-capable
//! connection beside a transaction-mode pooler). A lost hub session ends the
//! serving loop, and every reconnect goes through the replay: the relay
//! replays the rows it missed, or tells every resumable topic the relay lost
//! events; see [`super::bridge_replay`]. `NOTIFY` and the outbox writes are
//! ordinary statements on the main pool.
//!
//! The same subscription also carries [`CACHE_INVALIDATION_CHANNEL`], whose
//! payload is a cache domain; see [`super::cache_invalidation`].
//!
//! The payload is only the row id, well under the ~8 KB `NOTIFY` limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_database::{HubEvent, HubSubscription, NotificationHub};
use systemprompt_traits::CacheEpochs;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::bridge_handle::{EventBridgeHandle, RelayStatus, StatusCell};
use super::bridge_replay::ReplayCursor;
use super::cache_invalidation::{
    CACHE_INVALIDATION_CHANNEL, invalidate_all_cache_domains, receive_cache_invalidation,
};

use super::origin::relays_row;
use super::repository::{EventOutboxRepository, OutboxRow};
use super::routing::{EventBroadcasters, EventRouter, OUTBOX_CHANNEL, OutboxChannel};
use systemprompt_identifiers::{EventOutboxId, InstanceId};

const OUTBOX_RETENTION: Duration = Duration::from_secs(3600);
const PRUNE_INTERVAL: Duration = Duration::from_secs(300);

enum Served {
    Stopped,
    Lost,
    Relistened,
}

#[derive(Debug, Clone)]
pub struct PostgresEventBridge {
    hub: NotificationHub,
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
        hub: NotificationHub,
    ) -> Self {
        Self {
            outbox: EventOutboxRepository::new(pool, instance_id),
            hub,
            epochs,
            broadcasters,
        }
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

    pub(super) async fn run(self, status: &StatusCell, cancel: CancellationToken) {
        let mut subscription = self
            .hub
            .subscribe(&[OUTBOX_CHANNEL, CACHE_INVALIDATION_CHANNEL]);
        let mut prune_tick = tokio::time::interval(PRUNE_INTERVAL);
        prune_tick.tick().await;
        let mut cursor = ReplayCursor::default();
        let mut listening = false;

        loop {
            if !listening {
                self.epochs.set_relay_live(false);
                status.set(RelayStatus::Reconnecting);
                if !Self::await_listening(&mut subscription, &cancel).await {
                    break;
                }
            }
            invalidate_all_cache_domains(&self.epochs);
            self.recover(&mut cursor, OUTBOX_RETENTION).await;
            self.epochs.set_relay_live(true);
            status.set(RelayStatus::Listening);
            info!(
                channel = OUTBOX_CHANNEL,
                "event bridge: listening for cross-replica events"
            );

            match self
                .serve(&mut subscription, &mut prune_tick, &mut cursor, &cancel)
                .await
            {
                Served::Stopped => break,
                Served::Lost => listening = false,
                Served::Relistened => listening = true,
            }
        }
        self.epochs.set_relay_live(false);
        status.set(RelayStatus::Stopped);
        info!("event bridge: stopped");
    }

    async fn await_listening(
        subscription: &mut HubSubscription,
        cancel: &CancellationToken,
    ) -> bool {
        loop {
            tokio::select! {
                () = cancel.cancelled() => return false,
                event = subscription.recv() => {
                    if matches!(event, HubEvent::Listening { .. }) {
                        return true;
                    }
                },
            }
        }
    }

    async fn serve(
        &self,
        subscription: &mut HubSubscription,
        prune_tick: &mut tokio::time::Interval,
        cursor: &mut ReplayCursor,
        cancel: &CancellationToken,
    ) -> Served {
        loop {
            tokio::select! {
                () = cancel.cancelled() => return Served::Stopped,
                event = subscription.recv() => match event {
                    HubEvent::Notification(notification)
                        if notification.channel.as_ref() == CACHE_INVALIDATION_CHANNEL =>
                    {
                        receive_cache_invalidation(&self.epochs, &notification.payload);
                    },
                    HubEvent::Notification(notification) => {
                        self.deliver(&EventOutboxId::new(notification.payload.as_ref()), cursor).await;
                    },
                    HubEvent::Lost => {
                        warn!("event bridge: listener session lost; replaying on reconnect");
                        return Served::Lost;
                    },
                    HubEvent::Listening { .. } => {
                        warn!("event bridge: listener session replaced; replaying");
                        return Served::Relistened;
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
        if !relays_row(
            self.outbox.instance_id(),
            &row.origin_instance_id,
            row.deliver_to_origin,
        ) {
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
