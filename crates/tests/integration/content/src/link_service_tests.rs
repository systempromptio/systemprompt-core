//! DB-backed tests for `LinkGenerationService` — exercises the high-level
//! link creation helpers (social, internal content, external CTA) which
//! delegate to the underlying repository.

use systemprompt_content::models::{LinkType, UtmParams};
use systemprompt_content::services::link::{GenerateLinkParams, LinkGenerationService};
use systemprompt_database::DbPool;
use systemprompt_identifiers::CampaignId;

async fn try_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn service_new_succeeds() {
    let Some(db) = try_db().await else {
        return;
    };
    assert!(LinkGenerationService::new(&db).is_ok());
}

#[tokio::test]
async fn generate_link_with_minimal_params_creates_row() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = LinkGenerationService::new(&db).expect("service");
    let params = GenerateLinkParams {
        target_url: "https://example.com/landing".to_owned(),
        link_type: LinkType::Redirect,
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        expires_at: None,
    };
    let link = svc.generate_link(params).await.expect("generate");
    assert_eq!(link.target_url, "https://example.com/landing");
    assert!(!link.short_code.is_empty());

    svc.delete_link(&link.id).await.expect("cleanup");
}

#[tokio::test]
async fn generate_link_with_utm_persists_utm_json() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = LinkGenerationService::new(&db).expect("service");
    let utm = UtmParams {
        source: Some("twitter".to_owned()),
        medium: Some("social".to_owned()),
        campaign: Some("launch".to_owned()),
        term: None,
        content: None,
    };
    let params = GenerateLinkParams {
        target_url: "https://example.com/post".to_owned(),
        link_type: LinkType::Both,
        campaign_id: Some(CampaignId::new("c-launch".to_owned())),
        campaign_name: Some("Launch".to_owned()),
        source_content_id: None,
        source_page: None,
        utm_params: Some(utm),
        link_text: Some("Read more".to_owned()),
        link_position: Some("hero".to_owned()),
        expires_at: None,
    };
    let link = svc.generate_link(params).await.expect("generate");
    assert!(
        link.utm_params.as_ref().is_some_and(|s| s.contains("twitter")),
        "utm_params should contain the source token; got {:?}",
        link.utm_params
    );

    svc.delete_link(&link.id).await.ok();
}

#[tokio::test]
async fn generate_social_media_link_round_trips_through_get_by_short_code() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = LinkGenerationService::new(&db).expect("service");
    let link = svc
        .generate_social_media_link(
            "https://example.com/post",
            "linkedin",
            "release-week",
            None,
        )
        .await
        .expect("social link");

    let fetched = svc
        .get_link_by_short_code(&link.short_code)
        .await
        .expect("query")
        .expect("present");
    assert_eq!(fetched.id, link.id);

    svc.delete_link(&link.id).await.ok();
}

#[tokio::test]
async fn delete_link_via_service_removes_row() {
    let Some(db) = try_db().await else {
        return;
    };
    let svc = LinkGenerationService::new(&db).expect("service");
    let link = svc
        .generate_social_media_link(
            "https://example.com/x",
            "x",
            "campaign-x",
            None,
        )
        .await
        .expect("social link");
    let removed = svc.delete_link(&link.id).await.expect("delete");
    assert!(removed);

    let fetched = svc
        .get_link_by_short_code(&link.short_code)
        .await
        .expect("query");
    assert!(fetched.is_none());
}

#[tokio::test]
async fn build_trackable_url_appends_short_code_path() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;
    let link = CampaignLink {
        id: LinkId::generate(),
        short_code: "abc123".to_owned(),
        target_url: "https://target".to_owned(),
        link_type: "redirect".to_owned(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: None,
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };
    let url = LinkGenerationService::build_trackable_url(&link, "https://systemprompt.io");
    assert!(url.contains("abc123"));
    assert!(url.starts_with("https://systemprompt.io"));
}

#[tokio::test]
async fn inject_utm_params_appends_query_string() {
    let utm = UtmParams {
        source: Some("twitter".to_owned()),
        medium: Some("social".to_owned()),
        campaign: Some("c1".to_owned()),
        term: None,
        content: None,
    };
    let url = LinkGenerationService::inject_utm_params("https://example.com/a", &utm);
    assert!(url.contains("utm_source=twitter"));
    assert!(url.contains("utm_medium=social"));
    assert!(url.contains("utm_campaign=c1"));
}
