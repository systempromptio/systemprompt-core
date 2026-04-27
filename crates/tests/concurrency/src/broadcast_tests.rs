use std::sync::Arc;
use systemprompt_events::{Broadcaster, GenericBroadcaster};
use systemprompt_identifiers::UserId;
use systemprompt_models::A2AEvent;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

fn test_user_id(n: usize) -> UserId {
    UserId::new(format!("user-{n}"))
}

fn test_event() -> A2AEvent {
    use systemprompt_identifiers::{ContextId, TaskId};
    use systemprompt_models::events::payloads::a2a::TaskStatusUpdatePayload;

    A2AEvent::TaskStatusUpdate {
        timestamp: chrono::Utc::now(),
        payload: TaskStatusUpdatePayload {
            task_id: TaskId::generate(),
            context_id: ContextId::generate(),
            state: systemprompt_models::a2a::TaskState::Working,
            message: None,
        },
    }
}

#[tokio::test]
async fn concurrent_register_unregister_no_panic() {
    let broadcaster: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let mut join_set = JoinSet::new();
    let user = test_user_id(1);

    for i in 0..100 {
        let b = broadcaster.clone();
        let u = user.clone();
        join_set.spawn(async move {
            let conn_id = format!("conn-{i}");
            let (tx, _rx) = mpsc::unbounded_channel();
            b.register(&u, &conn_id, tx).await;
            b.unregister(&u, &conn_id).await;
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    assert_eq!(broadcaster.connection_count(&user).await, 0);
}

#[tokio::test]
async fn concurrent_broadcast_during_registration_storm() {
    let broadcaster: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let user = test_user_id(1);
    let mut join_set = JoinSet::new();

    let b = broadcaster.clone();
    let u = user.clone();
    join_set.spawn(async move {
        for _ in 0..50 {
            b.broadcast(&u, test_event()).await;
            tokio::task::yield_now().await;
        }
    });

    for i in 0..50 {
        let b = broadcaster.clone();
        let u = user.clone();
        join_set.spawn(async move {
            let conn_id = format!("storm-{i}");
            let (tx, _rx) = mpsc::unbounded_channel();
            b.register(&u, &conn_id, tx).await;
            tokio::task::yield_now().await;
            b.unregister(&u, &conn_id).await;
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }
}

#[tokio::test]
async fn concurrent_cleanup_under_contention() {
    let broadcaster: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let user = test_user_id(1);
    let mut receivers = Vec::new();

    for i in 0..100 {
        let (tx, rx) = mpsc::unbounded_channel();
        broadcaster.register(&user, &format!("conn-{i}"), tx).await;
        receivers.push(rx);
    }

    assert_eq!(broadcaster.connection_count(&user).await, 100);

    let mut join_set = JoinSet::new();
    for i in 0..100 {
        let b = broadcaster.clone();
        let u = user.clone();
        join_set.spawn(async move {
            b.unregister(&u, &format!("conn-{i}")).await;
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    assert_eq!(broadcaster.connection_count(&user).await, 0);
    assert_eq!(broadcaster.total_connections().await, 0);
}

#[tokio::test]
async fn broadcast_to_active_connections_succeeds() {
    let broadcaster: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let user = test_user_id(1);
    let mut receivers = Vec::new();

    for i in 0..10 {
        let (tx, rx) = mpsc::unbounded_channel();
        broadcaster.register(&user, &format!("conn-{i}"), tx).await;
        receivers.push(rx);
    }

    let sent = broadcaster.broadcast(&user, test_event()).await;
    assert_eq!(sent, 10);
}

#[tokio::test]
async fn broadcast_to_dropped_receivers_cleans_up() {
    let broadcaster: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let user = test_user_id(1);

    for i in 0..10 {
        let (tx, _rx) = mpsc::unbounded_channel();
        broadcaster.register(&user, &format!("conn-{i}"), tx).await;
    }

    let sent = broadcaster.broadcast(&user, test_event()).await;
    assert_eq!(sent, 0);
    assert_eq!(broadcaster.connection_count(&user).await, 0);
}

#[tokio::test]
async fn multiple_users_concurrent_operations() {
    let broadcaster: Arc<GenericBroadcaster<A2AEvent>> = Arc::new(GenericBroadcaster::new());
    let mut join_set = JoinSet::new();

    for u in 0..20 {
        let b = broadcaster.clone();
        join_set.spawn(async move {
            let user = test_user_id(u);
            for c in 0..5 {
                let (tx, _rx) = mpsc::unbounded_channel();
                b.register(&user, &format!("conn-{c}"), tx).await;
            }
            b.broadcast(&user, test_event()).await;
            for c in 0..5 {
                b.unregister(&user, &format!("conn-{c}")).await;
            }
        });
    }

    while let Some(result) = join_set.join_next().await {
        result.unwrap();
    }

    assert_eq!(broadcaster.total_connections().await, 0);
}
