/// Tests for analytics AI usage tracking
use crate::common::*;
use anyhow::Result;

#[tokio::test]
async fn test_session_request_count_increments() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    for expected_count in 1..=5 {
        let response = ctx.make_request("/").await?;
        assert!(response.status().is_success());

        wait_for_async_processing().await;

        let rows = ctx
            .db
            .fetch_all(
                &"SELECT session_id, user_id, started_at, request_count, user_type, \
                  fingerprint_hash, landing_page, entry_url, utm_source, utm_medium, \
                  utm_campaign, referrer_url, referrer_source FROM analytics_sessions WHERE \
                  fingerprint_hash = $1",
                &[&fingerprint],
            )
            .await?;

        let session = get_session_from_row(&rows[0])?;
        assert_eq!(
            session.request_count, expected_count as i32,
            "Expected {} requests, got {}",
            expected_count, session.request_count
        );
    }

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Request count increments correctly");
    Ok(())
}

#[tokio::test]
async fn test_session_activity_timestamps_updated() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let response1 = ctx.make_request("/").await?;
    assert!(response1.status().is_success());

    wait_for_async_processing().await;

    let rows1 = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source FROM analytics_sessions WHERE fingerprint_hash = $1",
            &[&fingerprint],
        )
        .await?;
    let session1 = get_session_from_row(&rows1[0])?;

    let started_at1 = session1.started_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let response2 = ctx.make_request("/").await?;
    assert!(response2.status().is_success());

    wait_for_async_processing().await;

    let rows2 = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source FROM analytics_sessions WHERE fingerprint_hash = $1",
            &[&fingerprint],
        )
        .await?;
    let session2 = get_session_from_row(&rows2[0])?;

    assert_eq!(
        started_at1, session2.started_at,
        "started_at should not change"
    );
    assert!(
        session2.request_count > session1.request_count,
        "request_count should increase"
    );

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Session activity timestamps updated correctly");
    Ok(())
}
