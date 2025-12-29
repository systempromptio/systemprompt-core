/// Tests for analytics events tracking
use crate::common::*;
use anyhow::Result;
use serde_json::Value;

#[tokio::test]
async fn test_page_view_event_recorded() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let response = ctx.make_request("/").await?;
    assert!(response.status().is_success());

    wait_for_async_processing().await;

    let session_rows = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source FROM analytics_sessions WHERE fingerprint_hash = $1",
            &[&fingerprint],
        )
        .await?;
    assert!(!session_rows.is_empty(), "Session not created");

    let session = get_session_from_row(&session_rows[0])?;

    let event_rows = ctx
        .db
        .fetch_all(
            &"SELECT event_id, session_id, event_type, metadata, created_at FROM analytics_events \
              WHERE session_id = $1 ORDER BY created_at",
            &[&session.session_id],
        )
        .await?;

    assert!(!event_rows.is_empty(), "No events recorded for session");

    let event_row = &event_rows[0];
    let event_type = event_row
        .get("event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(
        event_type == "page_view" || event_type == "navigation",
        "Expected page_view or navigation event, got: {}",
        event_type
    );

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Page view event recorded");
    Ok(())
}

#[tokio::test]
async fn test_event_metadata_persisted() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let response = ctx.make_request("/").await?;
    assert!(response.status().is_success());

    wait_for_async_processing().await;

    let session_rows = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source FROM analytics_sessions WHERE fingerprint_hash = $1",
            &[&fingerprint],
        )
        .await?;
    assert!(!session_rows.is_empty());

    let session = get_session_from_row(&session_rows[0])?;

    let event_rows = ctx
        .db
        .fetch_all(
            &"SELECT event_id, session_id, event_type, metadata, created_at FROM analytics_events \
              WHERE session_id = $1 ORDER BY created_at",
            &[&session.session_id],
        )
        .await?;

    assert!(!event_rows.is_empty(), "No events recorded");

    for event_row in event_rows {
        let metadata_value = event_row.get("metadata");
        assert!(metadata_value.is_some(), "Event metadata is missing");

        if let Some(Value::String(metadata_str)) = metadata_value {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(metadata_str);
            assert!(parsed.is_ok(), "Metadata is not valid JSON");
        }
    }

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Event metadata persisted correctly");
    Ok(())
}
