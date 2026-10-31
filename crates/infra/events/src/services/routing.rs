//! The [`EventBroadcasters`] set and the [`EventRouter`].
//!
//! One [`EventBroadcasters`] is built at the composition root and shared by
//! the router, the cross-replica bridge and the SSE endpoints; it holds the
//! fan-out point for each event kind. Service code routes through an
//! [`EventRouter`] rather than a broadcaster directly so that derived events
//! (e.g. AG-UI events also being placed on the unified context stream) are
//! routed consistently.
//!
//! # Cross-replica fan-out
//!
//! A broadcast reaches only the SSE connections held by the current
//! process. To deliver an event to subscribers attached to *other*
//! replicas, each `route_*` call also appends a row to the durable
//! `event_outbox` table and emits a Postgres `NOTIFY` on the
//! [`OUTBOX_CHANNEL`] channel. The [`crate::PostgresEventBridge`] running
//! on every replica consumes those notifications and re-injects the event
//! through the *local-only* path (`route_*_local`) — never back through
//! the outbox, which would loop forever.
//!
//! An [`EventRouter`] is a value built once at the composition root and
//! handed to whatever routes events: [`EventRouter::with_outbox`] relays
//! through the given pool, [`EventRouter::local_only`] never touches the
//! outbox and reports [`RelayOutcome::NotInstalled`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::{EventOutboxId, InstanceId, UserId};
use tracing::debug;

use super::repository::{EventOutboxRepository, OutboxInsert};
use super::{
    A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ContextBroadcaster, TopicBroadcaster,
    TopicEvent,
};
use crate::Broadcaster;
use crate::error::{RelayError, RelayOutcome, RouteOutcome};
use systemprompt_identifiers::Actor;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};

pub use super::outbox_channel::{NOTIFICATION_FACTS_CHANNEL, OUTBOX_CHANNEL, OutboxChannel};

/// The fan-out points for every event kind, built once and shared by the
/// router, the cross-replica bridge and the SSE endpoints of one process.
#[derive(Debug, Default)]
pub struct EventBroadcasters {
    pub context: Arc<ContextBroadcaster>,
    pub agui: Arc<AgUiBroadcaster>,
    pub a2a: Arc<A2ABroadcaster>,
    pub analytics: Arc<AnalyticsBroadcaster>,
    pub topics: Arc<TopicBroadcaster>,
}

impl EventBroadcasters {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn total_connections(&self) -> BroadcasterConnections {
        BroadcasterConnections {
            context: self.context.total_connections().await,
            agui: self.agui.total_connections().await,
            a2a: self.a2a.total_connections().await,
            analytics: self.analytics.total_connections().await,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcasterConnections {
    pub context: usize,
    pub agui: usize,
    pub a2a: usize,
    pub analytics: usize,
}

/// Routes events to local subscribers and, when built with an outbox, to the
/// other replicas.
#[derive(Debug, Clone)]
pub struct EventRouter {
    outbox: Option<EventOutboxRepository>,
    broadcasters: Arc<EventBroadcasters>,
}

impl EventRouter {
    #[must_use]
    pub const fn local_only(broadcasters: Arc<EventBroadcasters>) -> Self {
        Self {
            outbox: None,
            broadcasters,
        }
    }

    #[must_use]
    pub const fn with_outbox(
        pool: sqlx::PgPool,
        instance_id: InstanceId,
        broadcasters: Arc<EventBroadcasters>,
    ) -> Self {
        Self::from_outbox(EventOutboxRepository::new(pool, instance_id), broadcasters)
    }

    pub(super) const fn from_outbox(
        outbox: EventOutboxRepository,
        broadcasters: Arc<EventBroadcasters>,
    ) -> Self {
        Self {
            outbox: Some(outbox),
            broadcasters,
        }
    }

    #[must_use]
    pub const fn broadcasters(&self) -> &Arc<EventBroadcasters> {
        &self.broadcasters
    }

    async fn enqueue_outbox<T: serde::Serialize + Sync>(
        &self,
        channel: OutboxChannel,
        user_id: &UserId,
        event: &T,
    ) -> RelayOutcome {
        let Some(repo) = &self.outbox else {
            return RelayOutcome::NotInstalled;
        };
        match Self::relay(repo, channel, user_id, event).await {
            Ok(()) => RelayOutcome::Relayed,
            Err(error) => RelayOutcome::Failed(error),
        }
    }

    async fn relay<T: serde::Serialize + Sync>(
        repo: &EventOutboxRepository,
        channel: OutboxChannel,
        user_id: &UserId,
        event: &T,
    ) -> Result<(), RelayError> {
        let payload = serde_json::to_value(event).map_err(|source| RelayError::Serialize {
            channel: channel.as_str(),
            source,
        })?;
        let id = EventOutboxId::generate();
        let actor = Actor::user(user_id.clone());
        let persist = |source| RelayError::Persist {
            channel: channel.as_str(),
            source,
        };
        // Why: Postgres delivers a NOTIFY only when its transaction commits,
        // so the row and its announcement land together: a writer that dies
        // after the insert leaves no row its siblings were never told about.
        let mut tx = repo.begin().await.map_err(persist)?;
        let row = OutboxInsert {
            id: &id,
            channel,
            actor: &actor,
            payload: &payload,
        };
        repo.insert(&mut tx, row).await.map_err(persist)?;
        EventOutboxRepository::notify(&mut tx, &id)
            .await
            .map_err(RelayError::Notify)?;
        tx.commit().await.map_err(persist)
    }

    pub async fn route_agui_local(&self, user_id: &UserId, event: AgUiEvent) -> (usize, usize) {
        let event_type = event.event_type();
        let agui_count = self
            .broadcasters
            .agui
            .broadcast(user_id, event.clone())
            .await;
        let context_count = self
            .broadcasters
            .context
            .broadcast(user_id, ContextEvent::AgUi(event))
            .await;
        debug!(
            event_type = ?event_type,
            user_id = %user_id,
            agui_count = agui_count,
            context_count = context_count,
            "EventRouter: routed AG-UI event"
        );
        (agui_count, context_count)
    }

    pub async fn route_a2a_local(&self, user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        let a2a_count = self
            .broadcasters
            .a2a
            .broadcast(user_id, event.clone())
            .await;
        let context_count = self
            .broadcasters
            .context
            .broadcast(user_id, event.into())
            .await;
        (a2a_count, context_count)
    }

    pub async fn route_system_local(&self, user_id: &UserId, event: SystemEvent) -> usize {
        self.broadcasters
            .context
            .broadcast(user_id, ContextEvent::System(event))
            .await
    }

    pub async fn route_analytics_local(&self, user_id: &UserId, event: AnalyticsEvent) -> usize {
        self.broadcasters.analytics.broadcast(user_id, event).await
    }

    pub async fn route_agui(
        &self,
        user_id: &UserId,
        event: AgUiEvent,
    ) -> RouteOutcome<(usize, usize)> {
        let relay = self
            .enqueue_outbox(OutboxChannel::AgUi, user_id, &event)
            .await;
        let local = self.route_agui_local(user_id, event).await;
        RouteOutcome { local, relay }
    }

    pub async fn route_a2a(
        &self,
        user_id: &UserId,
        event: A2AEvent,
    ) -> RouteOutcome<(usize, usize)> {
        let relay = self
            .enqueue_outbox(OutboxChannel::A2A, user_id, &event)
            .await;
        let local = self.route_a2a_local(user_id, event).await;
        RouteOutcome { local, relay }
    }

    pub async fn route_system(&self, user_id: &UserId, event: SystemEvent) -> RouteOutcome<usize> {
        let relay = self
            .enqueue_outbox(OutboxChannel::System, user_id, &event)
            .await;
        let local = self.route_system_local(user_id, event).await;
        RouteOutcome { local, relay }
    }

    pub async fn route_analytics(
        &self,
        user_id: &UserId,
        event: AnalyticsEvent,
    ) -> RouteOutcome<usize> {
        let relay = self
            .enqueue_outbox(OutboxChannel::Analytics, user_id, &event)
            .await;
        let local = self.route_analytics_local(user_id, event).await;
        RouteOutcome { local, relay }
    }

    pub fn route_topic_local(&self, event: TopicEvent) -> usize {
        self.broadcasters.topics.publish(event)
    }

    pub async fn route_topic(&self, actor: &UserId, event: TopicEvent) -> RouteOutcome<usize> {
        let relay = self
            .enqueue_outbox(OutboxChannel::Topic, actor, &event)
            .await;
        let local = self.route_topic_local(event);
        RouteOutcome { local, relay }
    }
}
