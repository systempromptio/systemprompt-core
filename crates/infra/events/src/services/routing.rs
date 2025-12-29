use once_cell::sync::Lazy;
use systemprompt_identifiers::UserId;
use tracing::info;

use super::{A2ABroadcaster, AgUiBroadcaster, ContextBroadcaster};
use crate::Broadcaster;
use systemprompt_models::{A2AEvent, AgUiEvent, ContextEvent, SystemEvent};

pub static CONTEXT_BROADCASTER: Lazy<ContextBroadcaster> = Lazy::new(ContextBroadcaster::new);
pub static AGUI_BROADCASTER: Lazy<AgUiBroadcaster> = Lazy::new(AgUiBroadcaster::new);
pub static A2A_BROADCASTER: Lazy<A2ABroadcaster> = Lazy::new(A2ABroadcaster::new);

#[derive(Debug, Clone, Copy)]
pub struct EventRouter;

impl EventRouter {
    pub async fn route_agui(user_id: &UserId, event: AgUiEvent) -> (usize, usize) {
        let event_type = event.event_type();
        let agui_count = AGUI_BROADCASTER.broadcast(user_id, event.clone()).await;
        let context_count = CONTEXT_BROADCASTER
            .broadcast(user_id, ContextEvent::AgUi(event))
            .await;
        info!(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use systemprompt_identifiers::{ContextId, TaskId};
    use systemprompt_models::{a2a::TaskState, A2AEventBuilder, AgUiEvent, AgUiEventBuilder};

    fn test_user_id() -> UserId {
        UserId::new("test-router-user")
    }

    fn test_agui_event() -> AgUiEvent {
        AgUiEventBuilder::run_started(
            ContextId::new("test-context"),
            TaskId::new("test-task"),
            None,
        )
    }

    fn test_a2a_event() -> A2AEvent {
        A2AEventBuilder::task_status_update(
            TaskId::new("test-task"),
            ContextId::new("test-context"),
            TaskState::Working,
            Some("test message".to_string()),
        )
    }

    fn test_system_event() -> SystemEvent {
        systemprompt_models::SystemEventBuilder::heartbeat()
    }

    // ============================================================================
    // Static Broadcaster Initialization Tests
    // ============================================================================

    #[test]
    fn test_agui_broadcaster_initialized() {
        // Verify the lazy static is accessible
        let debug_str = format!("{:?}", *AGUI_BROADCASTER);
        assert!(debug_str.contains("GenericBroadcaster"));
    }

    #[test]
    fn test_a2a_broadcaster_initialized() {
        let debug_str = format!("{:?}", *A2A_BROADCASTER);
        assert!(debug_str.contains("GenericBroadcaster"));
    }

    #[test]
    fn test_context_broadcaster_initialized() {
        let debug_str = format!("{:?}", *CONTEXT_BROADCASTER);
        assert!(debug_str.contains("GenericBroadcaster"));
    }

    // ============================================================================
    // EventRouter Type Tests
    // ============================================================================

    #[test]
    fn test_event_router_is_debug() {
        let router = EventRouter;
        let debug_str = format!("{:?}", router);
        assert!(debug_str.contains("EventRouter"));
    }

    #[test]
    fn test_event_router_is_clone() {
        let router = EventRouter;
        let cloned = Clone::clone(&router);
        assert!(format!("{:?}", cloned).contains("EventRouter"));
    }

    #[test]
    fn test_event_router_is_copy() {
        let router = EventRouter;
        let copied = router;
        assert!(format!("{:?}", copied).contains("EventRouter"));
        // Original should still be usable (Copy trait)
        assert!(format!("{:?}", router).contains("EventRouter"));
    }

    // ============================================================================
    // Route AgUI Tests
    // ============================================================================

    #[tokio::test]
    async fn test_route_agui_returns_tuple() {
        let user_id = test_user_id();
        let event = test_agui_event();

        let result = EventRouter::route_agui(&user_id, event).await;

        // Should return (agui_count, context_count) tuple
        assert_eq!(result.0, 0); // No connections registered
        assert_eq!(result.1, 0);
    }

    #[tokio::test]
    async fn test_route_agui_with_registered_connection() {
        let user_id = UserId::new("agui-test-user");
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        AGUI_BROADCASTER.register(&user_id, "agui-conn", sender).await;

        let event = test_agui_event();
        let (agui_count, _context_count) = EventRouter::route_agui(&user_id, event).await;

        assert_eq!(agui_count, 1);
        assert!(receiver.recv().await.is_some());

        // Cleanup
        AGUI_BROADCASTER.unregister(&user_id, "agui-conn").await;
    }

    // ============================================================================
    // Route A2A Tests
    // ============================================================================

    #[tokio::test]
    async fn test_route_a2a_returns_tuple() {
        let user_id = test_user_id();
        let event = test_a2a_event();

        let result = EventRouter::route_a2a(&user_id, event).await;

        assert_eq!(result.0, 0);
        assert_eq!(result.1, 0);
    }

    #[tokio::test]
    async fn test_route_a2a_with_registered_connection() {
        let user_id = UserId::new("a2a-test-user");
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        A2A_BROADCASTER.register(&user_id, "a2a-conn", sender).await;

        let event = test_a2a_event();
        let (a2a_count, _context_count) = EventRouter::route_a2a(&user_id, event).await;

        assert_eq!(a2a_count, 1);
        assert!(receiver.recv().await.is_some());

        // Cleanup
        A2A_BROADCASTER.unregister(&user_id, "a2a-conn").await;
    }

    // ============================================================================
    // Route System Tests
    // ============================================================================

    #[tokio::test]
    async fn test_route_system_returns_count() {
        let user_id = test_user_id();
        let event = test_system_event();

        let count = EventRouter::route_system(&user_id, event).await;

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_route_system_with_registered_connection() {
        let user_id = UserId::new("system-test-user");
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        CONTEXT_BROADCASTER
            .register(&user_id, "context-conn", sender)
            .await;

        let event = test_system_event();
        let count = EventRouter::route_system(&user_id, event).await;

        assert_eq!(count, 1);
        assert!(receiver.recv().await.is_some());

        // Cleanup
        CONTEXT_BROADCASTER
            .unregister(&user_id, "context-conn")
            .await;
    }

    // ============================================================================
    // Cross-routing Tests (AgUI -> Context)
    // ============================================================================

    #[tokio::test]
    async fn test_route_agui_broadcasts_to_context() {
        let user_id = UserId::new("cross-route-user");
        let (context_sender, mut context_receiver) = tokio::sync::mpsc::unbounded_channel();

        // Register only on context broadcaster
        CONTEXT_BROADCASTER
            .register(&user_id, "context-only-conn", context_sender)
            .await;

        let event = test_agui_event();
        let (_agui_count, context_count) = EventRouter::route_agui(&user_id, event).await;

        // Should broadcast to context broadcaster as ContextEvent::AgUi
        assert_eq!(context_count, 1);
        assert!(context_receiver.recv().await.is_some());

        // Cleanup
        CONTEXT_BROADCASTER
            .unregister(&user_id, "context-only-conn")
            .await;
    }

    #[tokio::test]
    async fn test_route_a2a_broadcasts_to_context() {
        let user_id = UserId::new("a2a-cross-route-user");
        let (context_sender, mut context_receiver) = tokio::sync::mpsc::unbounded_channel();

        CONTEXT_BROADCASTER
            .register(&user_id, "a2a-context-conn", context_sender)
            .await;

        let event = test_a2a_event();
        let (_a2a_count, context_count) = EventRouter::route_a2a(&user_id, event).await;

        assert_eq!(context_count, 1);
        assert!(context_receiver.recv().await.is_some());

        // Cleanup
        CONTEXT_BROADCASTER
            .unregister(&user_id, "a2a-context-conn")
            .await;
    }

    // ============================================================================
    // Event Type Preservation Tests
    // ============================================================================

    #[tokio::test]
    async fn test_agui_event_type_preserved() {
        let event = test_agui_event();
        let event_type = event.event_type();
        assert_eq!(
            event_type,
            systemprompt_models::AgUiEventType::RunStarted
        );
    }

    #[tokio::test]
    async fn test_a2a_event_type_preserved() {
        let event = test_a2a_event();
        let event_type = event.event_type();
        assert_eq!(
            event_type,
            systemprompt_models::A2AEventType::TaskStatusUpdate
        );
    }

    #[tokio::test]
    async fn test_system_event_type_preserved() {
        let event = test_system_event();
        let event_type = event.event_type();
        assert_eq!(
            event_type,
            systemprompt_models::SystemEventType::Heartbeat
        );
    }
}
