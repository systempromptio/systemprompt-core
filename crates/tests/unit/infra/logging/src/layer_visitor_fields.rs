//! DB-backed tests for the field visitors behind [`DatabaseLayer`].
//!
//! The visitors are private to the layer, so every arm is driven through a
//! real attached layer and asserted against the `metadata` / attribution
//! columns the layer persists.

use std::time::Duration;

use systemprompt_logging::DatabaseLayer;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use tracing::{info, info_span};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

#[derive(Debug)]
struct Opaque {
    #[expect(dead_code, reason = "read only through the derived Debug rendering")]
    inner: &'static str,
}

async fn row_for_message(
    pool: &sqlx::PgPool,
    trace_id: &str,
    message: &str,
) -> Option<(String, Option<String>, Option<String>)> {
    sqlx::query!(
        "SELECT message, metadata, user_id FROM logs WHERE trace_id = $1 AND message = $2",
        trace_id,
        message
    )
    .fetch_optional(pool)
    .await
    .expect("query logs")
    .map(|r| (r.message, r.metadata, r.user_id))
}

async fn wait_for_rows(pool: &sqlx::PgPool, trace_id: &str, want: i64) -> i64 {
    let mut count = 0_i64;
    for _ in 0..200 {
        count = sqlx::query_scalar!("SELECT COUNT(*) FROM logs WHERE trace_id = $1", trace_id)
            .fetch_one(pool)
            .await
            .expect("count logs")
            .unwrap_or(0);
        if count >= want {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    count
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_visitor_preserves_scalar_field_types_and_strips_ansi_from_messages() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let raw = db.pool_arc().unwrap().as_ref().clone();
    let trace_id = format!("visitor-scalars-{}", uuid::Uuid::new_v4().simple());

    {
        let layer = DatabaseLayer::new(db.clone());
        let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = info_span!(
            "request",
            user_id = "visitor-user",
            session_id = "visitor-session",
            trace_id = trace_id.as_str(),
        );
        let _enter = span.enter();

        info!(
            signed = -7_i64,
            unsigned = 9_u64,
            wide_signed = -9_007_199_254_740_993_i128,
            wide_unsigned = 18_446_744_073_709_551_615_u128,
            ratio = 0.5_f64,
            flag = true,
            "scalars"
        );
        info!("\u{1b}[31mred\u{1b}[0m and \u{1b}plain");

        wait_for_rows(&raw, &trace_id, 2).await;
    }

    // Why: while the TRACE-level layer is active, the poll queries' own sqlx
    // events also persist under this span's trace, so the total row count is
    // machine-speed dependent — only the two authored events are asserted.
    assert!(wait_for_rows(&raw, &trace_id, 2).await >= 2);

    let (_, metadata, _) = row_for_message(&raw, &trace_id, "scalars")
        .await
        .expect("the scalar event must be persisted");
    let metadata: serde_json::Value =
        serde_json::from_str(&metadata.expect("field-bearing event carries metadata"))
            .expect("metadata is JSON");

    assert_eq!(metadata["signed"], serde_json::json!(-7));
    assert_eq!(metadata["unsigned"], serde_json::json!(9));
    assert_eq!(metadata["ratio"], serde_json::json!(0.5));
    assert_eq!(metadata["flag"], serde_json::json!(true));
    assert!(
        metadata["wide_signed"].is_number(),
        "an i128 field must stay a JSON number, got {}",
        metadata["wide_signed"]
    );
    assert!(
        metadata["wide_unsigned"].is_number(),
        "a u128 field must stay a JSON number, got {}",
        metadata["wide_unsigned"]
    );

    let stripped = row_for_message(&raw, &trace_id, "red and plain").await;
    assert!(
        stripped.is_some(),
        "ANSI colour codes and bare escapes must be stripped from the stored message"
    );

    sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
        .execute(&raw)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redaction_applies_to_debug_rendered_fields_and_never_to_scalars() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let raw = db.pool_arc().unwrap().as_ref().clone();
    let trace_id = format!("visitor-redact-{}", uuid::Uuid::new_v4().simple());

    {
        let layer = DatabaseLayer::new(db.clone());
        let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = info_span!(
            "request",
            user_id = "redact-user",
            session_id = "redact-session",
            trace_id = trace_id.as_str(),
        );
        let _enter = span.enter();

        let secret = Opaque {
            inner: "hunter2-in-a-struct",
        };
        let harmless = Opaque { inner: "visible" };
        info!(
            password = ?secret,
            detail = ?harmless,
            api_key = 42_i64,
            "debug fields"
        );

        wait_for_rows(&raw, &trace_id, 1).await;
    }

    let (_, metadata, _) = row_for_message(&raw, &trace_id, "debug fields")
        .await
        .expect("the debug-field event must be persisted");
    let raw_metadata = metadata.expect("field-bearing event carries metadata");
    let metadata: serde_json::Value =
        serde_json::from_str(&raw_metadata).expect("metadata is JSON");

    assert_eq!(metadata["password"], serde_json::json!("[REDACTED]"));
    assert!(
        !raw_metadata.contains("hunter2-in-a-struct"),
        "a Debug-rendered secret must never reach the log store: {raw_metadata}"
    );
    assert!(
        metadata["detail"]
            .as_str()
            .is_some_and(|s| s.contains("visible")),
        "a non-sensitive Debug field must keep its rendering, got {}",
        metadata["detail"]
    );
    // A number cannot carry a secret, and blanking it would change the
    // recorded JSON type — the redaction list is deliberately name-based on
    // the string/debug arms only.
    assert_eq!(metadata["api_key"], serde_json::json!(42));

    sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
        .execute(&raw)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn span_attribution_fields_are_captured_when_recorded_via_debug() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let raw = db.pool_arc().unwrap().as_ref().clone();
    let trace_id = format!("visitor-span-debug-{}", uuid::Uuid::new_v4().simple());

    {
        let layer = DatabaseLayer::new(db.clone());
        let subscriber = tracing_subscriber::registry().with(layer.with_filter(LevelFilter::TRACE));
        let _guard = tracing::subscriber::set_default(subscriber);

        let user = Opaque {
            inner: "debug-span-user",
        };
        let span = info_span!(
            "request",
            user_id = ?user,
            session_id = "span-debug-session",
            trace_id = trace_id.as_str(),
        );
        let _enter = span.enter();
        info!("attributed via debug");

        wait_for_rows(&raw, &trace_id, 1).await;
    }

    let (_, _, user_id) = row_for_message(&raw, &trace_id, "attributed via debug")
        .await
        .expect("the event must be persisted");
    let user_id = user_id.expect("a span-supplied user_id must be persisted, not left NULL");
    assert!(
        user_id.contains("debug-span-user"),
        "a Debug-recorded span field must still attribute the event, got {user_id}"
    );

    sqlx::query!("DELETE FROM logs WHERE trace_id = $1", trace_id.as_str())
        .execute(&raw)
        .await
        .unwrap();
}
