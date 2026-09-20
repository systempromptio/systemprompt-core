use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_events::{
    A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, Broadcaster, CONTEXT_BROADCASTER,
    EventRouter, ToSse,
};
use systemprompt_identifiers::ConnectionId;
use systemprompt_models::AnalyticsEventBuilder;

pub(crate) async fn verify_capture(pool: &PgPool, outbox: &OutboxConsumer) {
    sqlx::raw_sql(include_str!(
        "../../../../infra/events/schema/reporting_capture.sql"
    ))
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TABLE user_sessions (session_id TEXT PRIMARY KEY)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../../../infra/logging/schema/log.sql"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../infra/logging/schema/analytics.sql"
    ))
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../infra/logging/schema/reporting_capture.sql"
    ))
    .execute(pool)
    .await
    .unwrap();

    let user = crate::unique_user_id("reporting-isolation");
    let connection = ConnectionId::generate();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(16);
    assert!(
        A2A_BROADCASTER
            .register(&user, &connection, sender.clone())
            .await
    );
    assert!(
        AGUI_BROADCASTER
            .register(&user, &connection, sender.clone())
            .await
    );
    assert!(
        ANALYTICS_BROADCASTER
            .register(&user, &connection, sender.clone())
            .await
    );
    assert!(
        CONTEXT_BROADCASTER
            .register(&user, &connection, sender)
            .await
    );

    let mut notifications = sqlx::postgres::PgListener::connect_with(pool)
        .await
        .unwrap();
    notifications
        .listen(systemprompt_events::OUTBOX_CHANNEL)
        .await
        .unwrap();

    let mut rollback = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO logs(id,level,module,message,user_id) VALUES ('rolled-back','INFO','capture','rollback',$1)")
        .bind(user.as_str()).execute(&mut *rollback).await.unwrap();
    let rolled_back_notification: String = sqlx::query_scalar(
        "SELECT id FROM event_outbox WHERE consumer = 'analytics_reporting' AND fact->'data'->>'key' = 'rolled-back'",
    )
    .fetch_one(&mut *rollback)
    .await
    .unwrap();
    assert!(outbox.claim("analytics_reporting").await.unwrap().is_none());
    rollback.rollback().await.unwrap();
    assert!(outbox.claim("analytics_reporting").await.unwrap().is_none());
    assert_no_notification(&mut notifications, &rolled_back_notification).await;

    sqlx::query("INSERT INTO logs(id,level,module,message,user_id,metadata) VALUES ('captured','INFO','capture','committed',$1,'private metadata')")
        .bind(user.as_str()).execute(pool).await.unwrap();
    let committed_notification: String = sqlx::query_scalar(
        "SELECT id FROM event_outbox WHERE consumer = 'analytics_reporting' AND fact->'data'->>'key' = 'captured'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    recv_notification(&mut notifications, &committed_notification).await;
    drop(notifications);
    let first = next_fact(outbox).await;
    assert_eq!(first["source"], "logs");
    assert_eq!(first["key"], "captured");
    assert_eq!(first["deleted"], false);
    assert_eq!(first["row"]["message"], "committed");
    assert!(first["row"]["session_id"].is_null());
    assert!(first["row"].get("metadata").is_none());

    sqlx::query("UPDATE logs SET metadata = 'changed private metadata' WHERE id = 'captured'")
        .execute(pool)
        .await
        .unwrap();
    assert!(outbox.claim("analytics_reporting").await.unwrap().is_none());
    sqlx::query("UPDATE logs SET user_id = NULL, message = 'corrected' WHERE id = 'captured'")
        .execute(pool)
        .await
        .unwrap();
    let correction = next_fact(outbox).await;
    assert!(correction["row"]["user_id"].is_null());
    assert_eq!(correction["row"]["message"], "corrected");
    assert!(correction["revision"].as_i64().unwrap() > first["revision"].as_i64().unwrap());

    sqlx::query("UPDATE logs SET id = 'renamed' WHERE id = 'captured'")
        .execute(pool)
        .await
        .unwrap();
    let renamed = [next_fact(outbox).await, next_fact(outbox).await];
    assert!(
        renamed
            .iter()
            .any(|fact| fact["key"] == "captured" && fact["deleted"] == true)
    );
    assert!(
        renamed
            .iter()
            .any(|fact| fact["key"] == "renamed" && fact["deleted"] == false)
    );
    sqlx::query("DELETE FROM logs WHERE id = 'renamed'")
        .execute(pool)
        .await
        .unwrap();
    let deleted = next_fact(outbox).await;
    assert_eq!(deleted["key"], "renamed");
    assert_eq!(deleted["deleted"], true);
    assert!(deleted["row"].is_null());

    let mut early = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO logs(id,level,module,message) VALUES ('early','INFO','capture','early transaction')")
        .execute(&mut *early).await.unwrap();
    let mut late = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO logs(id,level,module,message) VALUES ('late','INFO','capture','late transaction')")
        .execute(&mut *late).await.unwrap();
    late.commit().await.unwrap();
    let late_fact = next_fact(outbox).await;
    assert_eq!(late_fact["key"], "late");
    early.commit().await.unwrap();
    let early_fact = next_fact(outbox).await;
    assert_eq!(early_fact["key"], "early");
    assert!(early_fact["revision"].as_i64().unwrap() < late_fact["revision"].as_i64().unwrap());
    assert!(outbox.claim("analytics_reporting").await.unwrap().is_none());

    let heartbeat = AnalyticsEventBuilder::heartbeat();
    EventRouter::route_analytics(&user, heartbeat.clone())
        .await
        .into_local_logged();
    let delivered = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        format!("{delivered:?}"),
        format!("{:?}", heartbeat.to_sse().unwrap())
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(150), receiver.recv())
            .await
            .is_err()
    );
    A2A_BROADCASTER.unregister(&user, &connection).await;
    AGUI_BROADCASTER.unregister(&user, &connection).await;
    ANALYTICS_BROADCASTER.unregister(&user, &connection).await;
    CONTEXT_BROADCASTER.unregister(&user, &connection).await;
}

async fn assert_no_notification(notifications: &mut sqlx::postgres::PgListener, rejected: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(100);
    loop {
        let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) else {
            return;
        };
        match tokio::time::timeout(remaining, notifications.recv()).await {
            Err(_) => return,
            Ok(Ok(notification)) => assert_ne!(notification.payload(), rejected),
            Ok(Err(error)) => panic!("notification receive failed: {error}"),
        }
    }
}

async fn recv_notification(notifications: &mut sqlx::postgres::PgListener, expected: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .expect("matching reporting notification was not delivered");
        let notification = tokio::time::timeout(remaining, notifications.recv())
            .await
            .expect("matching reporting notification was not delivered")
            .expect("notification receive failed");
        if notification.payload() == expected {
            return;
        }
    }
}

async fn next_fact(outbox: &OutboxConsumer) -> Value {
    let delivery = outbox
        .claim("analytics_reporting")
        .await
        .unwrap()
        .expect("pending reporting fact");
    let fact = delivery.fact::<Value>().unwrap();
    assert_eq!(fact.consumer, "analytics_reporting");
    assert_eq!(fact.kind, "reporting.row");
    assert_eq!(fact.version, 1);
    delivery.acknowledge().await.unwrap();
    fact.data
}
