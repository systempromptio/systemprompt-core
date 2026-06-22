//! DB-backed tests for [`LinkGenerationService`].
//!
//! Each test drives a link-minting method end to end (UTM assembly, short-code
//! generation, destination-type resolution, persistence) and reads the row back
//! through the service, asserting the stored shape and the public trackable
//! URL.

use systemprompt_content::models::{CreateContentParams, LinkType, UtmParams};
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::services::link::generation::GenerateContentLinkParams;
use systemprompt_content::{GenerateLinkParams, LinkGenerationService};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn seed_content(pool: &DbPool, source: &SourceId) -> ContentId {
    let repo = ContentRepository::new(pool).expect("repo");
    let params = CreateContentParams::new(
        format!("link-src-{}", Uuid::new_v4()),
        "Link Source".to_owned(),
        "desc".to_owned(),
        "body".to_owned(),
        source.clone(),
    );
    repo.create(&params).await.expect("seed content").id
}

async fn cleanup(pool: &DbPool, source: &SourceId) {
    let repo = ContentRepository::new(pool).expect("repo");
    repo.delete_by_source(source).await.expect("cleanup");
}

#[tokio::test]
async fn generate_external_link_persists_and_resolves_external() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let link = svc
        .generate_link(GenerateLinkParams {
            target_url: "https://external.example.com/landing".to_owned(),
            link_type: LinkType::Redirect,
            campaign_id: None,
            campaign_name: Some("Q1".to_owned()),
            source_content_id: None,
            source_page: None,
            utm_params: None,
            link_text: Some("Click me".to_owned()),
            link_position: None,
            expires_at: None,
        })
        .await
        .expect("generate");

    assert_eq!(link.target_url, "https://external.example.com/landing");
    assert_eq!(link.link_type, "redirect");
    assert_eq!(link.short_code.len(), 8);
    assert_eq!(link.destination_type.as_deref(), Some("external"));
    assert_eq!(link.link_text.as_deref(), Some("Click me"));

    let fetched = svc
        .get_link_by_short_code(&link.short_code)
        .await
        .expect("fetch")
        .expect("present");
    assert_eq!(fetched.id, link.id);

    let trackable = LinkGenerationService::build_trackable_url(&link, "https://my.site");
    assert_eq!(trackable, format!("https://my.site/r/{}", link.short_code));

    assert!(svc.delete_link(&link.id).await.expect("delete"));
}

#[tokio::test]
async fn generate_link_with_utm_serializes_params_and_internal_destination() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let utm = UtmParams {
        source: Some("newsletter".to_owned()),
        medium: Some("email".to_owned()),
        campaign: Some("launch".to_owned()),
        term: None,
        content: None,
    };

    let link = svc
        .generate_link(GenerateLinkParams {
            target_url: "https://systemprompt.io/pricing".to_owned(),
            link_type: LinkType::Utm,
            campaign_id: None,
            campaign_name: None,
            source_content_id: None,
            source_page: None,
            utm_params: Some(utm),
            link_text: None,
            link_position: None,
            expires_at: None,
        })
        .await
        .expect("generate");

    assert_eq!(link.destination_type.as_deref(), Some("internal"));
    let stored = link.utm_params.clone().expect("utm stored");
    assert!(stored.contains("newsletter"));
    assert!(stored.contains("launch"));

    // For non-redirect links the trackable URL is the raw target.
    let trackable = LinkGenerationService::build_trackable_url(&link, "https://my.site");
    assert_eq!(trackable, "https://systemprompt.io/pricing");

    assert!(svc.delete_link(&link.id).await.expect("delete"));
}

#[tokio::test]
async fn generate_social_media_link_sets_social_utm() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let source = SourceId::new(format!("gen-social-{}", Uuid::new_v4()));
    let content_id = seed_content(&pool, &source).await;
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let link = svc
        .generate_social_media_link(
            "https://example.org/share",
            "twitter",
            "spring-campaign",
            Some(content_id.clone()),
        )
        .await
        .expect("generate");

    assert_eq!(link.link_type, "both");
    assert_eq!(link.source_content_id.as_ref(), Some(&content_id));
    assert_eq!(link.campaign_name.as_deref(), Some("spring-campaign"));
    let utm = link.utm_params.expect("utm");
    assert!(utm.contains("twitter"));
    assert!(utm.contains("social"));

    svc.delete_link(&link.id).await.expect("delete");
    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn generate_internal_content_link_is_idempotent_on_source_and_target() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let source = SourceId::new(format!("gen-internal-{}", Uuid::new_v4()));
    let content_id = seed_content(&pool, &source).await;
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let source_page = format!("/blog/{}", Uuid::new_v4());
    let target = "https://systemprompt.io/docs";

    let first = svc
        .generate_internal_content_link(GenerateContentLinkParams {
            target_url: target,
            source_content_id: &content_id,
            source_page: &source_page,
            link_text: Some("Docs".to_owned()),
            link_position: None,
        })
        .await
        .expect("first");

    let second = svc
        .generate_internal_content_link(GenerateContentLinkParams {
            target_url: target,
            source_content_id: &content_id,
            source_page: &source_page,
            link_text: Some("Docs again".to_owned()),
            link_position: None,
        })
        .await
        .expect("second");

    assert_eq!(
        first.id, second.id,
        "duplicate source/target should reuse the existing link"
    );
    assert_eq!(first.link_type, "utm");

    svc.delete_link(&first.id).await.expect("delete");
    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn generate_external_cta_link_marks_cta_position() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let link = svc
        .generate_external_cta_link(
            "https://buy.example.com",
            "checkout",
            None,
            Some("Buy now".to_owned()),
        )
        .await
        .expect("generate");

    assert_eq!(link.link_position.as_deref(), Some("cta"));
    assert_eq!(link.link_type, "both");
    assert_eq!(link.destination_type.as_deref(), Some("external"));
    let utm = link.utm_params.expect("utm");
    assert!(utm.contains("cta"));

    svc.delete_link(&link.id).await.expect("delete");
}

#[tokio::test]
async fn generate_external_content_link_is_a_redirect_share() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let source = SourceId::new(format!("gen-extcontent-{}", Uuid::new_v4()));
    let content_id = seed_content(&pool, &source).await;
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let source_page = format!("/p/{}", Uuid::new_v4());
    let link = svc
        .generate_external_content_link(GenerateContentLinkParams {
            target_url: "https://x.com/intent",
            source_content_id: &content_id,
            source_page: &source_page,
            link_text: None,
            link_position: None,
        })
        .await
        .expect("generate");

    assert_eq!(link.link_type, "redirect");
    assert_eq!(link.campaign_name.as_deref(), Some("Social Share"));
    assert_eq!(link.source_page.as_deref(), Some(source_page.as_str()));

    let fetched = svc
        .get_link_by_id(&link.id)
        .await
        .expect("by id")
        .expect("present");
    assert_eq!(fetched.short_code, link.short_code);

    svc.delete_link(&link.id).await.expect("delete");
    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn get_link_by_short_code_missing_is_none() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let svc = LinkGenerationService::new(&pool).expect("svc");

    let result = svc
        .get_link_by_short_code(&format!("missing{}", Uuid::new_v4().simple()))
        .await
        .expect("query");
    assert!(result.is_none());
}

#[test]
fn inject_utm_params_appends_with_correct_separator() {
    let utm = UtmParams {
        source: Some("src".to_owned()),
        medium: Some("med".to_owned()),
        campaign: None,
        term: None,
        content: None,
    };
    let plain = LinkGenerationService::inject_utm_params("https://a.test/page", &utm);
    assert!(plain.starts_with("https://a.test/page?"));
    assert!(plain.contains("utm_source=src"));

    let with_query = LinkGenerationService::inject_utm_params("https://a.test/page?x=1", &utm);
    assert!(with_query.contains("?x=1&"));
}

#[test]
fn inject_utm_params_empty_returns_url_unchanged() {
    let utm = UtmParams {
        source: None,
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };
    let out = LinkGenerationService::inject_utm_params("https://a.test/page", &utm);
    assert_eq!(out, "https://a.test/page");
}
