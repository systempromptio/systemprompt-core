//! DB-sink tests for content-based scrubbing and caps in the field visitor.

use std::time::Duration;

use systemprompt_logging::DatabaseLayer;
use systemprompt_logging::sanitize::{MAX_FIELD_BYTES, MAX_MESSAGE_BYTES};
use systemprompt_test_fixtures::test_db_pool;
use tracing::{info, info_span};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

async fn rows_for(pool: &sqlx::PgPool, trace_id: &str, marker: &str) -> Vec<(String, String)> {
    for _ in 0..200 {
        let rows = sqlx::query!(
            "SELECT message, metadata FROM logs WHERE trace_id = $1 AND message LIKE $2",
            trace_id,
            format!("{marker}%")
        )
        .fetch_all(pool)
        .await
        .expect("query logs");
        if !rows.is_empty() {
            return rows
                .into_iter()
                .map(|r| (r.message, r.metadata.unwrap_or_default()))
                .collect();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Vec::new()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credential_bearing_values_and_oversized_fields_are_scrubbed_before_the_log_store() {
    let db = test_db_pool().await;
    let raw = db.pool().as_ref().clone();
    let trace_id = format!("visitor-values-{}", uuid::Uuid::new_v4().simple());
    let marker = "scrub-marker";
    let body = "b".repeat(MAX_FIELD_BYTES * 4);
    let long_tail = "m".repeat(MAX_MESSAGE_BYTES * 2);

    {
        let (layer, _writer) = DatabaseLayer::new(&db);
        let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::INFO));
        let _guard = tracing::subscriber::set_default(subscriber);
        let span = info_span!(
            "request",
            user_id = "scrub-user",
            session_id = "scrub-session",
            trace_id = trace_id.as_str(),
        );
        let _enter = span.enter();

        let url = "https://svc:hook-pass-123@authz.internal/decide?key=query-secret-456";
        info!(
            url = %url,
            error = "connect to postgres://admin:db-pass-789@db/app failed",
            body = body.as_str(),
            "{marker} via https://u:message-pass-000@h/ {long_tail}"
        );
        rows_for(&raw, &trace_id, marker).await;
    }

    let rows = rows_for(&raw, &trace_id, marker).await;
    let (message, metadata) = rows.first().expect("the event must be persisted");
    for secret in [
        "hook-pass-123",
        "query-secret-456",
        "db-pass-789",
        "message-pass-000",
    ] {
        assert!(
            !metadata.contains(secret) && !message.contains(secret),
            "{secret} reached the log store"
        );
    }
    let fields: serde_json::Value = serde_json::from_str(metadata).expect("metadata is JSON");
    assert!(
        fields["url"]
            .as_str()
            .is_some_and(|u| u.contains("@authz.internal/decide"))
    );
    let stored_body = fields["body"].as_str().expect("body field");
    assert!(stored_body.len() < body.len() && stored_body.contains("[truncated "));
    assert!(message.len() < MAX_MESSAGE_BYTES + 64 && message.contains("[truncated "));

    sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
        .execute(&raw)
        .await
        .unwrap();
}
