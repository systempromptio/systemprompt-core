use systemprompt_events::{
    A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, Broadcaster, CONTEXT_BROADCASTER,
    EventRouter,
};
use systemprompt_identifiers::{ConnectionId, ContextId, TaskId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{
    A2AEventBuilder, AgUiEventBuilder, AnalyticsEventBuilder, SystemEventBuilder,
};
use systemprompt_test_fixtures::unique_user_id;

fn agui_event() -> systemprompt_models::AgUiEvent {
    AgUiEventBuilder::run_started(
        ContextId::new("00000000-0000-4000-8000-000000000001"),
        TaskId::new("local-task"),
        None,
    )
}

fn a2a_event() -> systemprompt_models::A2AEvent {
    A2AEventBuilder::task_status_update(
        TaskId::new("local-task"),
        ContextId::new("00000000-0000-4000-8000-000000000001"),
        TaskState::Working,
        None,
    )
}

fn system_event() -> systemprompt_models::SystemEvent {
    SystemEventBuilder::heartbeat()
}

fn analytics_event() -> systemprompt_models::AnalyticsEvent {
    AnalyticsEventBuilder::heartbeat()
}

#[tokio::test]
async fn route_agui_local_no_subscribers_returns_zeros() {
    let user_id = unique_user_id("agui-local");
    let (agui, ctx) = EventRouter::route_agui_local(&user_id, agui_event()).await;
    assert_eq!(agui, 0);
    assert_eq!(ctx, 0);
}

#[tokio::test]
async fn route_agui_local_delivers_to_agui_broadcaster() {
    let user_id = unique_user_id("agui-local-agui");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn = ConnectionId::new("lc-agui-1");
    AGUI_BROADCASTER.register(&user_id, &conn, tx).await;

    let (agui_count, _ctx_count) = EventRouter::route_agui_local(&user_id, agui_event()).await;
    assert_eq!(agui_count, 1);
    assert!(rx.recv().await.is_some());

    AGUI_BROADCASTER.unregister(&user_id, &conn).await;
}

#[tokio::test]
async fn route_agui_local_also_delivers_to_context_broadcaster() {
    let user_id = unique_user_id("agui-local-ctx");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn = ConnectionId::new("lc-ctx-1");
    CONTEXT_BROADCASTER.register(&user_id, &conn, tx).await;

    let (_agui_count, ctx_count) = EventRouter::route_agui_local(&user_id, agui_event()).await;
    assert_eq!(ctx_count, 1);
    assert!(rx.recv().await.is_some());

    CONTEXT_BROADCASTER.unregister(&user_id, &conn).await;
}

#[tokio::test]
async fn route_a2a_local_no_subscribers_returns_zeros() {
    let user_id = unique_user_id("a2a-local");
    let (a2a, ctx) = EventRouter::route_a2a_local(&user_id, a2a_event()).await;
    assert_eq!(a2a, 0);
    assert_eq!(ctx, 0);
}

#[tokio::test]
async fn route_a2a_local_delivers_to_a2a_broadcaster() {
    let user_id = unique_user_id("a2a-local-a2a");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn = ConnectionId::new("lc-a2a-1");
    A2A_BROADCASTER.register(&user_id, &conn, tx).await;

    let (a2a_count, _ctx) = EventRouter::route_a2a_local(&user_id, a2a_event()).await;
    assert_eq!(a2a_count, 1);
    assert!(rx.recv().await.is_some());

    A2A_BROADCASTER.unregister(&user_id, &conn).await;
}

#[tokio::test]
async fn route_a2a_local_also_delivers_to_context_broadcaster() {
    let user_id = unique_user_id("a2a-local-ctx");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn = ConnectionId::new("lc-a2a-ctx-1");
    CONTEXT_BROADCASTER.register(&user_id, &conn, tx).await;

    let (_a2a, ctx_count) = EventRouter::route_a2a_local(&user_id, a2a_event()).await;
    assert_eq!(ctx_count, 1);
    assert!(rx.recv().await.is_some());

    CONTEXT_BROADCASTER.unregister(&user_id, &conn).await;
}

#[tokio::test]
async fn route_system_local_no_subscribers_returns_zero() {
    let user_id = unique_user_id("sys-local");
    let count = EventRouter::route_system_local(&user_id, system_event()).await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn route_system_local_delivers_to_context_broadcaster() {
    let user_id = unique_user_id("sys-local-ctx");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn = ConnectionId::new("lc-sys-ctx-1");
    CONTEXT_BROADCASTER.register(&user_id, &conn, tx).await;

    let count = EventRouter::route_system_local(&user_id, system_event()).await;
    assert_eq!(count, 1);
    assert!(rx.recv().await.is_some());

    CONTEXT_BROADCASTER.unregister(&user_id, &conn).await;
}

#[tokio::test]
async fn route_analytics_local_no_subscribers_returns_zero() {
    let user_id = unique_user_id("analytics-local");
    let count = EventRouter::route_analytics_local(&user_id, analytics_event()).await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn route_analytics_local_delivers_to_analytics_broadcaster() {
    let user_id = unique_user_id("analytics-local-analytics");
    let (tx, mut rx) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn = ConnectionId::new("lc-analytics-1");
    ANALYTICS_BROADCASTER.register(&user_id, &conn, tx).await;

    let count = EventRouter::route_analytics_local(&user_id, analytics_event()).await;
    assert_eq!(count, 1);
    assert!(rx.recv().await.is_some());

    ANALYTICS_BROADCASTER.unregister(&user_id, &conn).await;
}

#[tokio::test]
async fn route_agui_local_multiple_context_subscribers() {
    let user_id = unique_user_id("agui-local-multi-ctx");
    let (tx1, mut rx1) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn1 = ConnectionId::new("lc-multi-ctx-1");
    let conn2 = ConnectionId::new("lc-multi-ctx-2");

    CONTEXT_BROADCASTER.register(&user_id, &conn1, tx1).await;
    CONTEXT_BROADCASTER.register(&user_id, &conn2, tx2).await;

    let (_agui, ctx_count) = EventRouter::route_agui_local(&user_id, agui_event()).await;
    assert_eq!(ctx_count, 2);
    assert!(rx1.recv().await.is_some());
    assert!(rx2.recv().await.is_some());

    CONTEXT_BROADCASTER.unregister(&user_id, &conn1).await;
    CONTEXT_BROADCASTER.unregister(&user_id, &conn2).await;
}

#[tokio::test]
async fn route_system_local_multiple_context_subscribers() {
    let user_id = unique_user_id("sys-local-multi");
    let (tx1, mut rx1) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel(systemprompt_events::SSE_BUFFER);
    let conn1 = ConnectionId::new("lc-sys-multi-1");
    let conn2 = ConnectionId::new("lc-sys-multi-2");

    CONTEXT_BROADCASTER.register(&user_id, &conn1, tx1).await;
    CONTEXT_BROADCASTER.register(&user_id, &conn2, tx2).await;

    let count = EventRouter::route_system_local(&user_id, system_event()).await;
    assert_eq!(count, 2);
    assert!(rx1.recv().await.is_some());
    assert!(rx2.recv().await.is_some());

    CONTEXT_BROADCASTER.unregister(&user_id, &conn1).await;
    CONTEXT_BROADCASTER.unregister(&user_id, &conn2).await;
}

#[test]
fn sse_buffer_size_is_expected() {
    assert_eq!(systemprompt_events::SSE_BUFFER, 1024);
}
