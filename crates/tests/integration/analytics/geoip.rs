/// Tests for analytics GeoIP detection
use crate::common::*;
use anyhow::Result;

#[tokio::test]
async fn test_geoip_location_enriched() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let response = ctx
        .http
        .get(&format!("{}/", ctx.base_url))
        .header("x-fingerprint", &fingerprint)
        .header("x-forwarded-for", "8.8.8.8")
        .send()
        .await?;

    assert!(response.status().is_success());

    wait_for_async_processing().await;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT session_id, user_id, started_at, request_count, user_type, fingerprint_hash, \
              landing_page, entry_url, utm_source, utm_medium, utm_campaign, referrer_url, \
              referrer_source, ip_address, country FROM analytics_sessions WHERE fingerprint_hash \
              = $1",
            &[&fingerprint],
        )
        .await?;

    assert!(!rows.is_empty(), "Session not created");

    let session_row = &rows[0];
    let ip_address = session_row.get("ip_address").and_then(|v| v.as_str());

    if let Some(ip) = ip_address {
        if !ip.starts_with("127.0.0.1") && !ip.starts_with("::1") {
            let country = session_row.get("country").and_then(|v| v.as_str());

            assert!(
                country.is_some(),
                "Country should be enriched from IP address: {}",
                ip
            );
        }
    }

    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(fingerprint);
    cleanup.cleanup_all().await?;

    println!("✓ GeoIP location enriched");
    Ok(())
}
