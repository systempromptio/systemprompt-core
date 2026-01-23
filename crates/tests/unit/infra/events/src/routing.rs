use systemprompt_events::{
    Broadcaster, EventRouter, A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER,
    CONTEXT_BROADCASTER,
};
use systemprompt_identifiers::{ContextId, TaskId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{
    A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, AnalyticsEvent, AnalyticsEventBuilder,
    SystemEvent,
};

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

fn test_analytics_event() -> AnalyticsEvent {
    AnalyticsEventBuilder::page_view(
        "test-session".to_string(),
        None,
        "/test-page".to_string(),
        None,
        Some("https://example.com/referrer".to_string()),
    )
}

#[test]
fn test_analytics_broadcaster_initialized() {
    let debug_str = format!("{:?}", *ANALYTICS_BROADCASTER);
    assert!(debug_str.contains("GenericBroadcaster"));
}

#[tokio::test]
async fn test_route_analytics_returns_count() {
    let user_id = test_user_id();
    let event = test_analytics_event();

    let count = EventRouter::route_analytics(&user_id, event).await;

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_route_analytics_with_registered_connection() {
    let user_id = UserId::new("analytics-test-user");
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    ANALYTICS_BROADCASTER
        .register(&user_id, "analytics-conn", sender)
        .await;

    let event = test_analytics_event();
    let count = EventRouter::route_analytics(&user_id, event).await;

    assert_eq!(count, 1);
    assert!(receiver.recv().await.is_some());

    ANALYTICS_BROADCASTER
        .unregister(&user_id, "analytics-conn")
        .await;
}

#[tokio::test]
async fn test_route_analytics_multiple_connections() {
    let user_id = UserId::new("analytics-multi-user");
    let (sender1, mut rx1) = tokio::sync::mpsc::unbounded_channel();
    let (sender2, mut rx2) = tokio::sync::mpsc::unbounded_channel();

    ANALYTICS_BROADCASTER
        .register(&user_id, "analytics-conn-1", sender1)
        .await;
    ANALYTICS_BROADCASTER
        .register(&user_id, "analytics-conn-2", sender2)
        .await;

    let event = test_analytics_event();
    let count = EventRouter::route_analytics(&user_id, event).await;

    assert_eq!(count, 2);
    assert!(rx1.recv().await.is_some());
    assert!(rx2.recv().await.is_some());

    ANALYTICS_BROADCASTER
        .unregister(&user_id, "analytics-conn-1")
        .await;
    ANALYTICS_BROADCASTER
        .unregister(&user_id, "analytics-conn-2")
        .await;
}

#[tokio::test]
async fn test_analytics_broadcaster_connection_count() {
    let user_id = UserId::new("analytics-count-user");
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

    assert_eq!(ANALYTICS_BROADCASTER.connection_count(&user_id).await, 0);

    ANALYTICS_BROADCASTER
        .register(&user_id, "count-conn", sender)
        .await;

    assert_eq!(ANALYTICS_BROADCASTER.connection_count(&user_id).await, 1);

    ANALYTICS_BROADCASTER
        .unregister(&user_id, "count-conn")
        .await;
}

#[tokio::test]
async fn test_analytics_event_timestamp_exists() {
    let event = test_analytics_event();
    let timestamp = event.timestamp();
    assert!(timestamp.timestamp() > 0);
}

#[tokio::test]
async fn test_route_analytics_heartbeat_event() {
    let user_id = UserId::new("heartbeat-test-user");
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    ANALYTICS_BROADCASTER
        .register(&user_id, "heartbeat-conn", sender)
        .await;

    let heartbeat = AnalyticsEventBuilder::heartbeat();
    let count = EventRouter::route_analytics(&user_id, heartbeat).await;
    assert_eq!(count, 1);
    assert!(receiver.recv().await.is_some());

    ANALYTICS_BROADCASTER
        .unregister(&user_id, "heartbeat-conn")
        .await;
}

#[tokio::test]
async fn test_route_analytics_session_ended_event() {
    let user_id = UserId::new("session-end-user");
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

    ANALYTICS_BROADCASTER
        .register(&user_id, "session-end-conn", sender)
        .await;

    let session_end = AnalyticsEventBuilder::session_ended(
        "test-session".to_string(),
        120000,
        10,
        20,
    );
    let count = EventRouter::route_analytics(&user_id, session_end).await;
    assert_eq!(count, 1);
    assert!(receiver.recv().await.is_some());

    ANALYTICS_BROADCASTER
        .unregister(&user_id, "session-end-conn")
        .await;
}
