/// Tests for analytics endpoint tracking
use crate::common::*;
use anyhow::Result;

#[tokio::test]
async fn test_endpoint_requests_recorded() -> Result<()> {
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

    let req_rows = ctx
        .db
        .fetch_all(
            &"SELECT request_id, session_id, endpoint_path, http_method, response_status, \
              response_time_ms, created_at FROM analytics_endpoint_requests WHERE session_id = $1 \
              ORDER BY created_at",
            &[&session.session_id],
        )
        .await?;

    assert!(!req_rows.is_empty(), "No endpoint requests logged");

    let first_request = &req_rows[0];
    let endpoint_path = first_request
        .get("endpoint_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(!endpoint_path.is_empty(), "Endpoint path is empty");

    let http_method = first_request
        .get("http_method")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert_eq!(http_method, "GET", "Expected GET method");

    let response_status = first_request
        .get("response_status")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    assert!(
        response_status >= 200,
        "Invalid response status: {}",
        response_status
    );

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Endpoint requests recorded");
    Ok(())
}

#[tokio::test]
async fn test_response_time_measured() -> Result<()> {
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

    let req_rows = ctx
        .db
        .fetch_all(
            &"SELECT request_id, session_id, endpoint_path, http_method, response_status, \
              response_time_ms, created_at FROM analytics_endpoint_requests WHERE session_id = $1 \
              ORDER BY created_at",
            &[&session.session_id],
        )
        .await?;

    assert!(!req_rows.is_empty());

    for request_row in req_rows {
        let response_time = request_row
            .get("response_time_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);

        assert!(
            response_time >= 0,
            "Response time is negative: {}",
            response_time
        );
    }

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Response times measured");
    Ok(())
}
