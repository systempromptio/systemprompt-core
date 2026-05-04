//! Static event broadcasters and the [`EventRouter`] facade.
//!
//! The four `LazyLock<…>` statics are the canonical fan-out points for
//! each event kind in the system; service code should always go through
//! [`EventRouter`] rather than reaching for the underlying broadcaster
//! directly so that derived events (e.g. AG-UI events also being placed on
//! the unified context stream) are routed consistently.

use std::sync::LazyLock;
use systemprompt_identifiers::UserId;
use tracing::debug;

use super::{A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ContextBroadcaster};
use crate::Broadcaster;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};

/// Process-global broadcaster for the unified context stream.
pub static CONTEXT_BROADCASTER: LazyLock<ContextBroadcaster> =
    LazyLock::new(ContextBroadcaster::new);
/// Process-global broadcaster for AG-UI front-end events.
pub static AGUI_BROADCASTER: LazyLock<AgUiBroadcaster> = LazyLock::new(AgUiBroadcaster::new);
/// Process-global broadcaster for A2A protocol events.
pub static A2A_BROADCASTER: LazyLock<A2ABroadcaster> = LazyLock::new(A2ABroadcaster::new);
/// Process-global broadcaster for analytics events.
pub static ANALYTICS_BROADCASTER: LazyLock<AnalyticsBroadcaster> =
    LazyLock::new(AnalyticsBroadcaster::new);

/// Stateless dispatcher that fans events out to the appropriate static
/// broadcasters and (where relevant) onto the unified context stream.
#[derive(Debug, Clone, Copy)]
pub struct EventRouter;

impl EventRouter {
    /// Routes an AG-UI event to both the dedicated AG-UI broadcaster and
    /// the unified context stream.
    ///
    /// Returns `(agui_delivered, context_delivered)` connection counts.
    pub async fn route_agui(user_id: &UserId, event: AgUiEvent) -> (usize, usize) {
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

    /// Routes an A2A protocol event to the A2A broadcaster and the unified
    /// context stream.
    ///
    /// Returns `(a2a_delivered, context_delivered)` connection counts.
    pub async fn route_a2a(user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        let a2a_count = A2A_BROADCASTER.broadcast(user_id, event.clone()).await;
        let context_count = CONTEXT_BROADCASTER.broadcast(user_id, event.into()).await;
        (a2a_count, context_count)
    }

    /// Routes a system event onto the unified context stream and returns
    /// the number of connections it was delivered to.
    pub async fn route_system(user_id: &UserId, event: SystemEvent) -> usize {
        CONTEXT_BROADCASTER
            .broadcast(user_id, ContextEvent::System(event))
            .await
    }

    /// Routes an analytics event to the analytics broadcaster and returns
    /// the number of connections it was delivered to.
    pub async fn route_analytics(user_id: &UserId, event: AnalyticsEvent) -> usize {
        ANALYTICS_BROADCASTER.broadcast(user_id, event).await
    }
}
