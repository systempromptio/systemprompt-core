use std::sync::Arc;
use systemprompt_events::{Broadcaster, EventBroadcasters, EventRouter};
use systemprompt_identifiers::{ConnectionId, ContextId, TaskId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{
    A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, AnalyticsEvent, AnalyticsEventBuilder,
    SystemEvent,
};
use systemprompt_test_fixtures::{fixture_user_id, unique_user_id};

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";

fn test_user_id() -> UserId {
    fixture_user_id()
}

fn test_agui_event() -> AgUiEvent {
    AgUiEventBuilder::run_started(
        ContextId::try_new(TEST_CONTEXT_ID_A).expect("valid ContextId"),
        TaskId::new("test-task"),
        None,
    )
}

fn test_a2a_event() -> A2AEvent {
    A2AEventBuilder::task_status_update(
        TaskId::new("test-task"),
        ContextId::try_new(TEST_CONTEXT_ID_A).expect("valid ContextId"),
        TaskState::Working,
        Some("test message".to_string()),
    )
}

fn test_system_event() -> SystemEvent {
    systemprompt_models::SystemEventBuilder::heartbeat()
}


#[tokio::test]
async fn test_route_agui_returns_tuple() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = test_user_id();
    let event = test_agui_event();

    let result = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_agui(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(result.0, 0);
    assert_eq!(result.1, 0);
}

#[tokio::test]
async fn test_route_agui_with_registered_connection() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .agui
        .register(&user_id, &ConnectionId::new("agui-conn"), sender)
        .await;

    let event = test_agui_event();
    let (agui_count, _context_count) = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_agui(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(agui_count, 1);
    let _ = receiver
        .recv()
        .await
        .expect("receiver.recv().await should be present");

    broadcasters
        .agui
        .unregister(&user_id, &ConnectionId::new("agui-conn"))
        .await;
}

#[tokio::test]
async fn test_route_a2a_returns_tuple() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = test_user_id();
    let event = test_a2a_event();

    let result = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_a2a(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(result.0, 0);
    assert_eq!(result.1, 0);
}

#[tokio::test]
async fn test_route_a2a_with_registered_connection() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .a2a
        .register(&user_id, &ConnectionId::new("a2a-conn"), sender)
        .await;

    let event = test_a2a_event();
    let (a2a_count, _context_count) = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_a2a(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(a2a_count, 1);
    let _ = receiver
        .recv()
        .await
        .expect("receiver.recv().await should be present");

    broadcasters
        .a2a
        .unregister(&user_id, &ConnectionId::new("a2a-conn"))
        .await;
}

#[tokio::test]
async fn test_route_system_returns_count() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = test_user_id();
    let event = test_system_event();

    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_system(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_route_system_with_registered_connection() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .context
        .register(&user_id, &ConnectionId::new("context-conn"), sender)
        .await;

    let event = test_system_event();
    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_system(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(count, 1);
    let _ = receiver
        .recv()
        .await
        .expect("receiver.recv().await should be present");

    broadcasters
        .context
        .unregister(&user_id, &ConnectionId::new("context-conn"))
        .await;
}

#[tokio::test]
async fn test_route_agui_broadcasts_to_context() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (context_sender, mut context_receiver) =
        tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .context
        .register(
            &user_id,
            &ConnectionId::new("context-only-conn"),
            context_sender,
        )
        .await;

    let event = test_agui_event();
    let (_agui_count, context_count) = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_agui(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(context_count, 1);
    let _ = context_receiver
        .recv()
        .await
        .expect("context_receiver.recv().await should be present");

    broadcasters
        .context
        .unregister(&user_id, &ConnectionId::new("context-only-conn"))
        .await;
}

#[tokio::test]
async fn test_route_a2a_broadcasts_to_context() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (context_sender, mut context_receiver) =
        tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .context
        .register(
            &user_id,
            &ConnectionId::new("a2a-context-conn"),
            context_sender,
        )
        .await;

    let event = test_a2a_event();
    let (_a2a_count, context_count) = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_a2a(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(context_count, 1);
    let _ = context_receiver
        .recv()
        .await
        .expect("context_receiver.recv().await should be present");

    broadcasters
        .context
        .unregister(&user_id, &ConnectionId::new("a2a-context-conn"))
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
        "test-session".to_string().into(),
        None,
        "/test-page".to_string(),
        None,
        Some("https://example.com/referrer".to_string()),
    )
}


#[tokio::test]
async fn test_route_analytics_returns_count() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = test_user_id();
    let event = test_analytics_event();

    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_analytics(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_route_analytics_with_registered_connection() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .analytics
        .register(&user_id, &ConnectionId::new("analytics-conn"), sender)
        .await;

    let event = test_analytics_event();
    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_analytics(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(count, 1);
    let _ = receiver
        .recv()
        .await
        .expect("receiver.recv().await should be present");

    broadcasters
        .analytics
        .unregister(&user_id, &ConnectionId::new("analytics-conn"))
        .await;
}

#[tokio::test]
async fn test_route_analytics_multiple_connections() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender1, mut rx1) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let (sender2, mut rx2) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .analytics
        .register(&user_id, &ConnectionId::new("analytics-conn-1"), sender1)
        .await;
    broadcasters
        .analytics
        .register(&user_id, &ConnectionId::new("analytics-conn-2"), sender2)
        .await;

    let event = test_analytics_event();
    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_analytics(&user_id, event)
        .await
        .into_local_logged();

    assert_eq!(count, 2);
    let _ = rx1
        .recv()
        .await
        .expect("rx1.recv().await should be present");
    let _ = rx2
        .recv()
        .await
        .expect("rx2.recv().await should be present");

    broadcasters
        .analytics
        .unregister(&user_id, &ConnectionId::new("analytics-conn-1"))
        .await;
    broadcasters
        .analytics
        .unregister(&user_id, &ConnectionId::new("analytics-conn-2"))
        .await;
}

#[tokio::test]
async fn test_analytics_broadcaster_connection_count() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, _receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    assert_eq!(broadcasters.analytics.connection_count(&user_id).await, 0);

    broadcasters
        .analytics
        .register(&user_id, &ConnectionId::new("count-conn"), sender)
        .await;

    assert_eq!(broadcasters.analytics.connection_count(&user_id).await, 1);

    broadcasters
        .analytics
        .unregister(&user_id, &ConnectionId::new("count-conn"))
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
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .analytics
        .register(&user_id, &ConnectionId::new("heartbeat-conn"), sender)
        .await;

    let heartbeat = AnalyticsEventBuilder::heartbeat();
    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_analytics(&user_id, heartbeat)
        .await
        .into_local_logged();
    assert_eq!(count, 1);
    let _ = receiver
        .recv()
        .await
        .expect("receiver.recv().await should be present");

    broadcasters
        .analytics
        .unregister(&user_id, &ConnectionId::new("heartbeat-conn"))
        .await;
}

#[tokio::test]
async fn test_route_analytics_session_ended_event() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let user_id = unique_user_id("user-id");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);

    broadcasters
        .analytics
        .register(&user_id, &ConnectionId::new("session-end-conn"), sender)
        .await;

    let session_end =
        AnalyticsEventBuilder::session_ended("test-session".to_string().into(), 120000, 10, 20);
    let count = EventRouter::local_only(Arc::clone(&broadcasters))
        .route_analytics(&user_id, session_end)
        .await
        .into_local_logged();
    assert_eq!(count, 1);
    let _ = receiver
        .recv()
        .await
        .expect("receiver.recv().await should be present");

    broadcasters
        .analytics
        .unregister(&user_id, &ConnectionId::new("session-end-conn"))
        .await;
}
