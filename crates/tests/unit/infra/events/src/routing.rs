use systemprompt_events::{
    Broadcaster, EventRouter, A2A_BROADCASTER, AGUI_BROADCASTER, CONTEXT_BROADCASTER,
};
use systemprompt_identifiers::{ContextId, TaskId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, SystemEvent};

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

#[test]
fn test_agui_broadcaster_initialized() {
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
    assert!(format!("{:?}", router).contains("EventRouter"));
}

#[tokio::test]
async fn test_route_agui_returns_tuple() {
    let user_id = test_user_id();
    let event = test_agui_event();

    let result = EventRouter::route_agui(&user_id, event).await;

    assert_eq!(result.0, 0);
    assert_eq!(result.1, 0);
}

#[tokio::test]
async fn test_route_agui_with_registered_connection() {
    let user_id = UserId::new("agui-test-user");
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    AGUI_BROADCASTER
        .register(&user_id, "agui-conn", sender)
        .await;

    let event = test_agui_event();
    let (agui_count, _context_count) = EventRouter::route_agui(&user_id, event).await;

    assert_eq!(agui_count, 1);
    assert!(receiver.recv().await.is_some());

    AGUI_BROADCASTER.unregister(&user_id, "agui-conn").await;
}

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

    A2A_BROADCASTER.unregister(&user_id, "a2a-conn").await;
}

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

    CONTEXT_BROADCASTER
        .unregister(&user_id, "context-conn")
        .await;
}

#[tokio::test]
async fn test_route_agui_broadcasts_to_context() {
    let user_id = UserId::new("cross-route-user");
    let (context_sender, mut context_receiver) = tokio::sync::mpsc::unbounded_channel();

    CONTEXT_BROADCASTER
        .register(&user_id, "context-only-conn", context_sender)
        .await;

    let event = test_agui_event();
    let (_agui_count, context_count) = EventRouter::route_agui(&user_id, event).await;

    assert_eq!(context_count, 1);
    assert!(context_receiver.recv().await.is_some());

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

    CONTEXT_BROADCASTER
        .unregister(&user_id, "a2a-context-conn")
        .await;
}

#[tokio::test]
async fn test_agui_event_type_preserved() {
    let event = test_agui_event();
    let event_type = event.event_type();
    assert_eq!(event_type, systemprompt_models::AgUiEventType::RunStarted);
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
    assert_eq!(event_type, systemprompt_models::SystemEventType::Heartbeat);
}
