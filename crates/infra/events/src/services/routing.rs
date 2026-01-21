use std::sync::LazyLock;
use systemprompt_identifiers::UserId;
use tracing::debug;

use super::{A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ContextBroadcaster};
use crate::Broadcaster;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};

pub static CONTEXT_BROADCASTER: LazyLock<ContextBroadcaster> =
    LazyLock::new(ContextBroadcaster::new);
pub static AGUI_BROADCASTER: LazyLock<AgUiBroadcaster> = LazyLock::new(AgUiBroadcaster::new);
pub static A2A_BROADCASTER: LazyLock<A2ABroadcaster> = LazyLock::new(A2ABroadcaster::new);
pub static ANALYTICS_BROADCASTER: LazyLock<AnalyticsBroadcaster> =
    LazyLock::new(AnalyticsBroadcaster::new);

#[derive(Debug, Clone, Copy)]
pub struct EventRouter;

impl EventRouter {
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

    pub async fn route_a2a(user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        let a2a_count = A2A_BROADCASTER.broadcast(user_id, event.clone()).await;
        let context_count = CONTEXT_BROADCASTER.broadcast(user_id, event.into()).await;
        (a2a_count, context_count)
    }

    pub async fn route_system(user_id: &UserId, event: SystemEvent) -> usize {
        CONTEXT_BROADCASTER
            .broadcast(user_id, ContextEvent::System(event))
            .await
    }

    pub async fn route_analytics(user_id: &UserId, event: AnalyticsEvent) -> usize {
        ANALYTICS_BROADCASTER.broadcast(user_id, event).await
    }
}
