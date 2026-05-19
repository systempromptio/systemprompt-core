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

use std::sync::{LazyLock, OnceLock};
use systemprompt_identifiers::{EventOutboxId, UserId};
use tracing::{debug, error};

use super::{A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ContextBroadcaster};
use crate::Broadcaster;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};

/// Postgres `LISTEN`/`NOTIFY` channel used for the cross-replica relay.
pub const OUTBOX_CHANNEL: &str = "systemprompt_events";

pub static CONTEXT_BROADCASTER: LazyLock<ContextBroadcaster> =
    LazyLock::new(ContextBroadcaster::new);
pub static AGUI_BROADCASTER: LazyLock<AgUiBroadcaster> = LazyLock::new(AgUiBroadcaster::new);
pub static A2A_BROADCASTER: LazyLock<A2ABroadcaster> = LazyLock::new(A2ABroadcaster::new);
pub static ANALYTICS_BROADCASTER: LazyLock<AnalyticsBroadcaster> =
    LazyLock::new(AnalyticsBroadcaster::new);

static RELAY_POOL: OnceLock<sqlx::PgPool> = OnceLock::new();

/// The event kind carried by an `event_outbox` row, used to pick the
/// correct deserialization target on the consuming replica.
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
    /// Installs the Postgres pool used to persist outbox rows and emit
    /// `NOTIFY`. Idempotent: a second call is ignored. Called once by the
    /// [`crate::PostgresEventBridge`] at startup.
    pub fn install_relay(pool: sqlx::PgPool) {
        if RELAY_POOL.set(pool).is_err() {
            debug!("EventRouter relay pool already installed; ignoring");
        }
    }

    async fn enqueue_outbox<T: serde::Serialize + Sync>(
        channel: OutboxChannel,
        user_id: &UserId,
        event: &T,
    ) {
        let Some(pool) = RELAY_POOL.get() else {
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
        let insert = sqlx::query!(
            "INSERT INTO event_outbox (id, channel, user_id, payload) VALUES ($1, $2, $3, $4)",
            id.as_str(),
            channel.as_str(),
            user_id.as_str(),
            payload,
        )
        .execute(pool)
        .await;
        if let Err(e) = insert {
            error!(error = %e, channel = channel.as_str(), "failed to persist outbox row");
            return;
        }
        let notify = sqlx::query("SELECT pg_notify($1, $2)")
            .bind(OUTBOX_CHANNEL)
            .bind(id.as_str())
            .execute(pool)
            .await;
        if let Err(e) = notify {
            error!(error = %e, "failed to NOTIFY cross-replica relay");
        }
    }

    /// Fans an AG-UI event into the local broadcasters only. The relay
    /// uses this entry point to re-inject events received from other
    /// replicas without re-publishing them to the outbox.
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

    /// Fans an A2A event into the local broadcasters only.
    pub async fn route_a2a_local(user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        let a2a_count = A2A_BROADCASTER.broadcast(user_id, event.clone()).await;
        let context_count = CONTEXT_BROADCASTER.broadcast(user_id, event.into()).await;
        (a2a_count, context_count)
    }

    /// Fans a system event into the local context broadcaster only.
    pub async fn route_system_local(user_id: &UserId, event: SystemEvent) -> usize {
        CONTEXT_BROADCASTER
            .broadcast(user_id, ContextEvent::System(event))
            .await
    }

    /// Fans an analytics event into the local analytics broadcaster only.
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
