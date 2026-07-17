//! Static event broadcasters and the [`EventRouter`] facade.
//!
//! The four `LazyLock<…>` statics are the canonical fan-out points for
//! each event kind in the system; service code should always go through
//! [`EventRouter`] rather than reaching for the underlying broadcaster
//! directly so that derived events (e.g. AG-UI events also being placed on
//! the unified context stream) are routed consistently.
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
//! The relay pool is installed once at startup via
//! [`EventRouter::install_relay`]. Before installation (or in deployments
//! without Postgres) routing is local-only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{LazyLock, OnceLock};
use systemprompt_identifiers::{EventOutboxId, UserId};
use tracing::{debug, error};

use super::repository::EventOutboxRepository;
use super::{A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ContextBroadcaster};
use crate::Broadcaster;
use systemprompt_identifiers::Actor;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};

pub const OUTBOX_CHANNEL: &str = "systemprompt_events";

pub static CONTEXT_BROADCASTER: LazyLock<ContextBroadcaster> =
    LazyLock::new(ContextBroadcaster::new);
pub static AGUI_BROADCASTER: LazyLock<AgUiBroadcaster> = LazyLock::new(AgUiBroadcaster::new);
pub static A2A_BROADCASTER: LazyLock<A2ABroadcaster> = LazyLock::new(A2ABroadcaster::new);
pub static ANALYTICS_BROADCASTER: LazyLock<AnalyticsBroadcaster> =
    LazyLock::new(AnalyticsBroadcaster::new);

static OUTBOX_REPO: OnceLock<EventOutboxRepository> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxChannel {
    AgUi,
    A2A,
    System,
    Analytics,
}

impl OutboxChannel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgUi => "agui",
            Self::A2A => "a2a",
            Self::System => "system",
            Self::Analytics => "analytics",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agui" => Some(Self::AgUi),
            "a2a" => Some(Self::A2A),
            "system" => Some(Self::System),
            "analytics" => Some(Self::Analytics),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EventRouter;

impl EventRouter {
    /// Idempotent: a second call is ignored.
    pub fn install_relay(pool: sqlx::PgPool) {
        if OUTBOX_REPO.set(EventOutboxRepository::new(pool)).is_err() {
            debug!("EventRouter relay pool already installed; ignoring");
        }
    }

    async fn enqueue_outbox<T: serde::Serialize + Sync>(
        channel: OutboxChannel,
        user_id: &UserId,
        event: &T,
    ) {
        let Some(repo) = OUTBOX_REPO.get() else {
            return;
        };
        let payload = match serde_json::to_value(event) {
            Ok(value) => value,
            Err(e) => {
                error!(error = %e, channel = channel.as_str(), "failed to serialize event for outbox");
                return;
            },
        };
        let id = EventOutboxId::generate();
        let actor = Actor::user(user_id.clone());
        if let Err(e) = repo.insert(&id, channel, &actor, &payload).await {
            error!(error = %e, channel = channel.as_str(), "failed to persist outbox row");
            return;
        }
        if let Err(e) = repo.notify(&id).await {
            error!(error = %e, "failed to NOTIFY cross-replica relay");
        }
    }

    /// Local-only: re-injects relayed events without re-publishing to the
    /// outbox.
    pub async fn route_agui_local(user_id: &UserId, event: AgUiEvent) -> (usize, usize) {
        let event_type = event.event_type();
        let agui_count = AGUI_BROADCASTER.broadcast(user_id, event.clone()).await;
        let context_count = CONTEXT_BROADCASTER
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

    pub async fn route_a2a_local(user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        let a2a_count = A2A_BROADCASTER.broadcast(user_id, event.clone()).await;
        let context_count = CONTEXT_BROADCASTER.broadcast(user_id, event.into()).await;
        (a2a_count, context_count)
    }

    pub async fn route_system_local(user_id: &UserId, event: SystemEvent) -> usize {
        CONTEXT_BROADCASTER
            .broadcast(user_id, ContextEvent::System(event))
            .await
    }

    pub async fn route_analytics_local(user_id: &UserId, event: AnalyticsEvent) -> usize {
        ANALYTICS_BROADCASTER.broadcast(user_id, event).await
    }

    pub async fn route_agui(user_id: &UserId, event: AgUiEvent) -> (usize, usize) {
        Self::enqueue_outbox(OutboxChannel::AgUi, user_id, &event).await;
        Self::route_agui_local(user_id, event).await
    }

    pub async fn route_a2a(user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        Self::enqueue_outbox(OutboxChannel::A2A, user_id, &event).await;
        Self::route_a2a_local(user_id, event).await
    }

    pub async fn route_system(user_id: &UserId, event: SystemEvent) -> usize {
        Self::enqueue_outbox(OutboxChannel::System, user_id, &event).await;
        Self::route_system_local(user_id, event).await
    }

    pub async fn route_analytics(user_id: &UserId, event: AnalyticsEvent) -> usize {
        Self::enqueue_outbox(OutboxChannel::Analytics, user_id, &event).await;
        Self::route_analytics_local(user_id, event).await
    }
}
