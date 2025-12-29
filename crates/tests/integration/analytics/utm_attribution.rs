/// Tests for analytics UTM attribution tracking
use crate::common::*;
use anyhow::Result;

#[tokio::test]
async fn test_utm_parameters_extracted() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let path = "/?utm_source=facebook&utm_medium=paid&utm_campaign=awareness";
    let response = ctx.make_request(path).await?;
    assert!(response.status().is_success());

    wait_for_async_processing().await;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source FROM analytics_sessions WHERE fingerprint_hash = $1",
            &[&fingerprint],
        )
        .await?;

    assert!(!rows.is_empty(), "Session not created");
    let session = get_session_from_row(&rows[0])?;

    assert_eq!(session.utm_source, Some("facebook".to_string()));
    assert_eq!(session.utm_medium, Some("paid".to_string()));
    assert_eq!(session.utm_campaign, Some("awareness".to_string()));

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ UTM parameters extracted and tracked");
    Ok(())
}

#[tokio::test]
async fn test_referrer_information_captured() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let response = ctx
        .http
        .get(&format!("{}/", ctx.base_url))
        .header("x-fingerprint", &fingerprint)
        .header("referer", "https://google.com/search?q=test")
        .send()
        .await?;

    assert!(response.status().is_success());

    wait_for_async_processing().await;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source FROM analytics_sessions WHERE fingerprint_hash = $1",
            &[&fingerprint],
        )
        .await?;

    assert!(!rows.is_empty(), "Session not created");

    let session = get_session_from_row(&rows[0])?;
    assert!(
        session.referrer_url.is_some() || session.referrer_source.is_some(),
        "Referrer information not captured"
    );

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ Referrer information captured");
    Ok(())
}
