use crate::common::context::TestContext;
use anyhow::Result;
use serde_json::json;
use systemprompt_database::DatabaseProvider;
use uuid::Uuid;

#[tokio::test]
async fn test_link_generation() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/test",
            "link_type": "both",
            "campaign_name": "TEST_CAMPAIGN",
            "utm_source": "test",
            "utm_medium": "integration",
            "utm_campaign": "test_suite"
        }))
        .send()
        .await?;

    println!("Response status: {}", response.status());
    let body = response.text().await?;
    println!("Response body: {}", body);

    let json: serde_json::Value = serde_json::from_str(&body)?;

    assert!(json.get("link_id").is_some(), "Should have link_id");
    assert!(json.get("short_code").is_some(), "Should have short_code");
    assert!(
        json.get("redirect_url").is_some(),
        "Should have redirect_url"
    );
    assert!(json.get("full_url").is_some(), "Should have full_url");

    Ok(())
}

#[tokio::test]
async fn test_link_redirect() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // First, generate a link
    let gen_response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/redirect-test",
            "link_type": "both",
            "campaign_name": "TEST_REDIRECT"
        }))
        .send()
        .await?;

    let gen_json: serde_json::Value = gen_response.json().await?;
    let short_code = gen_json["short_code"].as_str().unwrap();

    // Test redirect
    let redirect_response = client
        .get(format!("{}/r/{}", ctx.base_url, short_code))
        .send()
        .await?;

    assert_eq!(
        redirect_response.status(),
        307,
        "Should return 307 redirect"
    );

    let location = redirect_response
        .headers()
        .get("location")
        .unwrap()
        .to_str()?;

    assert!(
        location.contains("example.com/redirect-test"),
        "Should redirect to target URL"
    );

    Ok(())
}

#[tokio::test]
async fn test_link_performance() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Generate a link
    let gen_response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/performance-test",
            "link_type": "both",
            "campaign_id": "TEST_PERF_CAMPAIGN",
            "campaign_name": "TEST_PERFORMANCE"
        }))
        .send()
        .await?;

    let gen_json: serde_json::Value = gen_response.json().await?;
    let link_id = gen_json["link_id"].as_str().unwrap();

    // Get performance metrics
    let perf_response = client
        .get(format!(
            "{}/api/v1/content/links/{}/performance",
            ctx.base_url, link_id
        ))
        .send()
        .await?;

    assert!(perf_response.status().is_success());

    let perf_json: serde_json::Value = perf_response.json().await?;

    assert!(perf_json.get("id").is_some());
    assert!(perf_json.get("click_count").is_some());
    assert!(perf_json.get("unique_click_count").is_some());
    assert!(perf_json.get("session_count").is_some());

    Ok(())
}

#[tokio::test]
async fn test_campaign_performance() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let campaign_id = "TEST_CAMPAIGN_ANALYTICS";

    // Generate multiple links for the campaign
    for i in 0..3 {
        client
            .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
            .json(&json!({
                "target_url": format!("https://example.com/post-{}", i),
                "link_type": "both",
                "campaign_id": campaign_id,
                "campaign_name": "TEST_CAMPAIGN_NAME"
            }))
            .send()
            .await?;
    }

    // Get campaign performance
    let response = client
        .get(format!(
            "{}/api/v1/content/links/campaigns/{}/performance",
            ctx.base_url, campaign_id
        ))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;

    assert_eq!(json["campaign_id"], campaign_id);
    assert!(json["link_count"].as_i64().unwrap() >= 3);

    Ok(())
}

#[tokio::test]
async fn test_content_journey() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Generate internal navigation links
    client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/blog/post-2",
            "link_type": "utm",
            "campaign_id": "TEST_JOURNEY",
            "campaign_name": "TEST_INTERNAL_NAV",
            "source_content_id": "post-1",
            "source_page": "/blog/post-1",
            "utm_source": "internal",
            "utm_medium": "content"
        }))
        .send()
        .await?;

    // Get journey map
    let response = client
        .get(format!(
            "{}/api/v1/content/links/journey?limit=50",
            ctx.base_url
        ))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;

    assert!(json.is_array());

    Ok(())
}

#[tokio::test]
async fn test_invalid_link_type() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com",
            "link_type": "invalid",
            "campaign_name": "TEST_ERROR"
        }))
        .send()
        .await?;

    assert_eq!(response.status(), 400);

    let json: serde_json::Value = response.json().await?;
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("Invalid link_type"));

    Ok(())
}

#[tokio::test]
async fn test_nonexistent_link_redirect() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let response = client
        .get(format!("{}/r/nonexistent123", ctx.base_url))
        .send()
        .await?;

    assert_eq!(response.status(), 404);

    Ok(())
}

/// Regression test for NULL timestamp binding bug
///
/// This test ensures that links can be created with NULL optional fields
/// (particularly expires_at as Option<DateTime<Utc>>).
///
/// Bug: Previously, binding Option<DateTime<Utc>> = None would bind as
/// None::<String>, causing PostgreSQL to reject it for TIMESTAMP columns.
///
/// Fix: Type-specific NULL variants (DbValue::NullTimestamp) ensure
/// PostgreSQL receives the correct type even for NULL values.
#[tokio::test]
async fn test_null_timestamp_binding_regression() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    // Generate link WITHOUT expires_at (NULL timestamp)
    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/null-test",
            "link_type": "both",
            "campaign_name": "NULL_REGRESSION_TEST"
            // Deliberately omitting expires_at to test NULL binding
        }))
        .send()
        .await?;

    println!("Response status: {}", response.status());
    let body = response.text().await?;
    println!("Response body: {}", body);

    // Should succeed (previously failed with "incorrect binary data format")
    let json: serde_json::Value = serde_json::from_str(&body)?;

    assert!(json.get("link_id").is_some(), "Link should be created");
    assert!(json.get("short_code").is_some(), "Should have short_code");

    let link_id = json["link_id"].as_str().unwrap();
    let short_code = json["short_code"].as_str().unwrap();

    let row = ctx
        .db
        .fetch_optional(
            &"SELECT link_id, short_code, target_url, click_count, expires_at, created_at FROM \
              content_links WHERE link_id = $1",
            &[&link_id],
        )
        .await?;

    assert!(row.is_some(), "Link should exist in database");
    let link_row = row.unwrap();

    // Verify expires_at is NULL in database
    let expires_at = link_row.get("expires_at");
    assert!(
        expires_at.is_none() || expires_at == Some(&serde_json::Value::Null),
        "expires_at should be NULL in database"
    );

    // Verify redirect still works with NULL timestamp
    let redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let redirect_response = redirect_client
        .get(format!("{}/r/{}", ctx.base_url, short_code))
        .send()
        .await?;

    assert_eq!(
        redirect_response.status(),
        307,
        "Redirect should work even with NULL expires_at"
    );

    Ok(())
}

/// Test creating a link WITH an expiration date
///
/// This ensures the opposite case works: when expires_at IS provided,
/// it should be properly stored as a TIMESTAMP value.
#[tokio::test]
async fn test_link_with_expiration() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let expires_at = "2026-12-31T23:59:59Z";

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/expiring-link",
            "link_type": "both",
            "campaign_name": "EXPIRING_LINK_TEST",
            "expires_at": expires_at
        }))
        .send()
        .await?;

    assert!(
        response.status().is_success(),
        "Should create link with expires_at"
    );

    let json: serde_json::Value = response.json().await?;
    let link_id = json["link_id"].as_str().unwrap();

    let row = ctx
        .db
        .fetch_optional(
            &"SELECT link_id, short_code, target_url, click_count, expires_at, created_at FROM \
              content_links WHERE link_id = $1",
            &[&link_id],
        )
        .await?;

    assert!(row.is_some(), "Link should exist");
    let link_row = row.unwrap();

    let stored_expires = link_row.get("expires_at");
    assert!(
        stored_expires.is_some() && stored_expires != Some(&serde_json::Value::Null),
        "expires_at should be stored as valid timestamp"
    );

    Ok(())
}

#[tokio::test]
async fn test_short_code_uniqueness() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let mut short_codes = std::collections::HashSet::new();

    for i in 0..10 {
        let response = client
            .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
            .json(&json!({
                "target_url": format!("https://example.com/unique-test-{}", i),
                "link_type": "redirect",
                "campaign_name": "UNIQUENESS_TEST"
            }))
            .send()
            .await?;

        assert!(response.status().is_success());

        let json: serde_json::Value = response.json().await?;
        let short_code = json["short_code"].as_str().unwrap().to_string();

        assert!(
            !short_codes.contains(&short_code),
            "Short code {} should be unique",
            short_code
        );
        short_codes.insert(short_code);
    }

    assert_eq!(short_codes.len(), 10, "Should have 10 unique short codes");

    println!("✓ Generated 10 unique short codes");
    Ok(())
}

#[tokio::test]
async fn test_link_destination_type_internal() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "/blog/internal-article",
            "link_type": "redirect",
            "campaign_name": "INTERNAL_DEST_TEST"
        }))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;
    let link_id = json["link_id"].as_str().unwrap();

    let row = ctx
        .db
        .fetch_optional(
            &"SELECT destination_type FROM content_links WHERE link_id = $1",
            &[&link_id],
        )
        .await?;

    assert!(row.is_some());
    let destination_type = row.unwrap().get("destination_type");
    assert_eq!(
        destination_type.and_then(|v| v.as_str()),
        Some("internal"),
        "Slash-prefixed URLs should be internal"
    );

    println!("✓ Internal destination type detected correctly");
    Ok(())
}

#[tokio::test]
async fn test_link_destination_type_external() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://github.com/anthropics/claude-code",
            "link_type": "redirect",
            "campaign_name": "EXTERNAL_DEST_TEST"
        }))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;
    let link_id = json["link_id"].as_str().unwrap();

    let row = ctx
        .db
        .fetch_optional(
            &"SELECT destination_type FROM content_links WHERE link_id = $1",
            &[&link_id],
        )
        .await?;

    assert!(row.is_some());
    let destination_type = row.unwrap().get("destination_type");
    assert_eq!(
        destination_type.and_then(|v| v.as_str()),
        Some("external"),
        "External URLs should be marked external"
    );

    println!("✓ External destination type detected correctly");
    Ok(())
}

#[tokio::test]
async fn test_link_utm_only_type() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/utm-test",
            "link_type": "utm",
            "campaign_name": "UTM_ONLY_TEST",
            "utm_source": "newsletter",
            "utm_medium": "email",
            "utm_campaign": "weekly_digest"
        }))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;

    assert!(json.get("link_id").is_some());
    assert!(json.get("short_code").is_some());
    assert!(json.get("full_url").is_some());

    let full_url = json["full_url"].as_str().unwrap();
    assert!(
        full_url.contains("utm_source=newsletter"),
        "Full URL should contain UTM params"
    );
    assert!(
        full_url.contains("utm_medium=email"),
        "Full URL should contain UTM medium"
    );

    println!("✓ UTM-only link generated correctly");
    Ok(())
}

#[tokio::test]
async fn test_link_with_source_content_tracking() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/tracked-link",
            "link_type": "both",
            "campaign_name": "CONTENT_TRACKING_TEST",
            "source_content_id": "blog-post-123",
            "source_page": "/blog/how-to-use-ai",
            "link_text": "Learn more",
            "link_position": "footer"
        }))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;
    let link_id = json["link_id"].as_str().unwrap();

    let row = ctx
        .db
        .fetch_optional(
            &"SELECT source_content_id, source_page, link_text, link_position FROM content_links WHERE link_id = $1",
            &[&link_id],
        )
        .await?;

    assert!(row.is_some());
    let link_row = row.unwrap();

    assert_eq!(
        link_row.get("source_content_id").and_then(|v| v.as_str()),
        Some("blog-post-123")
    );
    assert_eq!(
        link_row.get("source_page").and_then(|v| v.as_str()),
        Some("/blog/how-to-use-ai")
    );
    assert_eq!(
        link_row.get("link_text").and_then(|v| v.as_str()),
        Some("Learn more")
    );
    assert_eq!(
        link_row.get("link_position").and_then(|v| v.as_str()),
        Some("footer")
    );

    println!("✓ Source content tracking fields stored correctly");
    Ok(())
}

#[tokio::test]
async fn test_links_by_campaign_listing() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let campaign_id = format!("CAMPAIGN_LIST_{}", Uuid::new_v4());

    for i in 0..5 {
        client
            .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
            .json(&json!({
                "target_url": format!("https://example.com/campaign-link-{}", i),
                "link_type": "redirect",
                "campaign_id": campaign_id,
                "campaign_name": "CAMPAIGN_LISTING_TEST"
            }))
            .send()
            .await?;
    }

    let response = client
        .get(format!(
            "{}/api/v1/content/links/campaigns/{}/performance",
            ctx.base_url, campaign_id
        ))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;
    let link_count = json["link_count"].as_i64().unwrap_or(0);

    assert!(
        link_count >= 5,
        "Campaign should have at least 5 links (found {})",
        link_count
    );

    println!("✓ Campaign link listing works correctly");
    Ok(())
}

#[tokio::test]
async fn test_redirect_preserves_utm_params() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let gen_response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/redirect-utm",
            "link_type": "both",
            "campaign_name": "REDIRECT_UTM_TEST",
            "utm_source": "test",
            "utm_medium": "integration"
        }))
        .send()
        .await?;

    let gen_json: serde_json::Value = gen_response.json().await?;
    let short_code = gen_json["short_code"].as_str().unwrap();

    let redirect_response = client
        .get(format!("{}/r/{}", ctx.base_url, short_code))
        .send()
        .await?;

    assert_eq!(redirect_response.status(), 307);

    let location = redirect_response
        .headers()
        .get("location")
        .unwrap()
        .to_str()?;

    assert!(
        location.contains("utm_source=test"),
        "Redirect should preserve UTM source"
    );
    assert!(
        location.contains("utm_medium=integration"),
        "Redirect should preserve UTM medium"
    );

    println!("✓ Redirect preserves UTM parameters");
    Ok(())
}

#[tokio::test]
async fn test_link_click_count_increment() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let gen_response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/click-count-test",
            "link_type": "redirect",
            "campaign_name": "CLICK_COUNT_TEST"
        }))
        .send()
        .await?;

    let gen_json: serde_json::Value = gen_response.json().await?;
    let link_id = gen_json["link_id"].as_str().unwrap();
    let short_code = gen_json["short_code"].as_str().unwrap();

    for _ in 0..3 {
        client
            .get(format!("{}/r/{}", ctx.base_url, short_code))
            .send()
            .await?;
    }

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let perf_response = client
        .get(format!(
            "{}/api/v1/content/links/{}/performance",
            ctx.base_url, link_id
        ))
        .send()
        .await?;

    assert!(perf_response.status().is_success());

    let perf_json: serde_json::Value = perf_response.json().await?;
    let click_count = perf_json["click_count"].as_i64().unwrap_or(0);

    assert!(
        click_count >= 3,
        "Click count should be at least 3 (found {})",
        click_count
    );

    println!("✓ Click count incremented correctly");
    Ok(())
}

#[tokio::test]
async fn test_link_generation_with_all_utm_params() -> Result<()> {
    let ctx = TestContext::new().await?;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/v1/content/links/generate", ctx.base_url))
        .json(&json!({
            "target_url": "https://example.com/full-utm-test",
            "link_type": "utm",
            "campaign_name": "FULL_UTM_TEST",
            "utm_source": "google",
            "utm_medium": "cpc",
            "utm_campaign": "summer_sale",
            "utm_term": "ai tools",
            "utm_content": "banner_ad"
        }))
        .send()
        .await?;

    assert!(response.status().is_success());

    let json: serde_json::Value = response.json().await?;
    let full_url = json["full_url"].as_str().unwrap();

    assert!(full_url.contains("utm_source=google"));
    assert!(full_url.contains("utm_medium=cpc"));
    assert!(full_url.contains("utm_campaign=summer_sale"));
    assert!(full_url.contains("utm_term="));
    assert!(full_url.contains("utm_content=banner_ad"));

    println!("✓ All UTM parameters included in generated URL");
    Ok(())
}
