//! DB-backed tests for [`LinkAnalyticsService`].
//!
//! Seeds a campaign link (and content parent for journey-map coverage), records
//! clicks through the service, and asserts the maintained counters, first-click
//! semantics, per-link click listings, and campaign/journey aggregates.

use systemprompt_content::models::{CreateContentParams, LinkType, TrackClickParams};
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::{GenerateLinkParams, LinkAnalyticsService, LinkGenerationService};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CampaignId, ContentId, LinkId, SessionId, SourceId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn seed_content(pool: &DbPool, source: &SourceId) -> ContentId {
    let repo = ContentRepository::new(pool).expect("repo");
    let params = CreateContentParams::new(
        format!("an-src-{}", Uuid::new_v4()),
        "Analytics Source".to_owned(),
        "desc".to_owned(),
        "body".to_owned(),
        source.clone(),
    );
    repo.create(&params).await.expect("seed content").id
}

async fn seed_link(pool: &DbPool, campaign: &CampaignId, content_id: Option<ContentId>) -> LinkId {
    let svc = LinkGenerationService::new(pool).expect("gen");
    let link = svc
        .generate_link(GenerateLinkParams {
            target_url: format!("https://example.com/{}", Uuid::new_v4()),
            link_type: LinkType::Both,
            campaign_id: Some(campaign.clone()),
            campaign_name: Some("analytics-campaign".to_owned()),
            source_content_id: content_id,
            source_page: None,
            utm_params: None,
            link_text: None,
            link_position: None,
            expires_at: None,
        })
        .await
        .expect("seed link");
    link.id
}

fn track_params(link_id: &LinkId, session: &SessionId) -> TrackClickParams {
    TrackClickParams {
        link_id: link_id.clone(),
        session_id: session.clone(),
        user_id: None,
        context_id: None,
        task_id: None,
        referrer_page: Some("/blog".to_owned()),
        referrer_url: None,
        user_agent: Some("UnitTest/1.0".to_owned()),
        ip_address: Some("203.0.113.5".to_owned()),
        device_type: Some("desktop".to_owned()),
        country: Some("US".to_owned()),
    }
}

async fn cleanup(pool: &DbPool, link_id: &LinkId, source: Option<&SourceId>) {
    LinkGenerationService::new(pool)
        .expect("gen")
        .delete_link(link_id)
        .await
        .expect("delete link");
    if let Some(source) = source {
        ContentRepository::new(pool)
            .expect("repo")
            .delete_by_source(source)
            .await
            .expect("cleanup content");
    }
}

#[tokio::test]
async fn track_click_first_then_repeat_updates_counters() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let campaign = CampaignId::new(format!("camp-{}", Uuid::new_v4()));
    let link_id = seed_link(&pool, &campaign, None).await;
    let svc = LinkAnalyticsService::new(&pool).expect("svc");

    let session = SessionId::new(format!("sess-{}", Uuid::new_v4()));

    let first = svc
        .track_click(&track_params(&link_id, &session))
        .await
        .expect("first click");
    assert_eq!(first.is_first_click, Some(true));
    assert_eq!(first.is_conversion, Some(false));
    assert_eq!(first.device_type.as_deref(), Some("desktop"));
    assert_eq!(first.link_id, link_id);

    let second = svc
        .track_click(&track_params(&link_id, &session))
        .await
        .expect("second click");
    assert_eq!(
        second.is_first_click,
        Some(false),
        "same session should not be a first click again"
    );

    let perf = svc
        .get_link_performance(&link_id)
        .await
        .expect("perf")
        .expect("present");
    assert_eq!(perf.click_count, 2);
    assert_eq!(perf.unique_click_count, 1);
    assert_eq!(perf.conversion_count, 0);

    let clicks = svc
        .get_link_clicks(&link_id, None, None)
        .await
        .expect("clicks");
    assert_eq!(clicks.len(), 2);

    cleanup(&pool, &link_id, None).await;
}

#[tokio::test]
async fn distinct_sessions_each_count_as_unique() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let campaign = CampaignId::new(format!("camp-{}", Uuid::new_v4()));
    let link_id = seed_link(&pool, &campaign, None).await;
    let svc = LinkAnalyticsService::new(&pool).expect("svc");

    for _ in 0..3 {
        let session = SessionId::new(format!("sess-{}", Uuid::new_v4()));
        svc.track_click(&track_params(&link_id, &session))
            .await
            .expect("click");
    }

    let perf = svc
        .get_link_performance(&link_id)
        .await
        .expect("perf")
        .expect("present");
    assert_eq!(perf.click_count, 3);
    assert_eq!(perf.unique_click_count, 3);

    cleanup(&pool, &link_id, None).await;
}

#[tokio::test]
async fn campaign_performance_aggregates_links() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let campaign = CampaignId::new(format!("camp-{}", Uuid::new_v4()));
    let link_a = seed_link(&pool, &campaign, None).await;
    let link_b = seed_link(&pool, &campaign, None).await;
    let svc = LinkAnalyticsService::new(&pool).expect("svc");

    svc.track_click(&track_params(
        &link_a,
        &SessionId::new(format!("s-{}", Uuid::new_v4())),
    ))
    .await
    .expect("click a");
    svc.track_click(&track_params(
        &link_b,
        &SessionId::new(format!("s-{}", Uuid::new_v4())),
    ))
    .await
    .expect("click b");

    let perf = svc
        .get_campaign_performance(&campaign)
        .await
        .expect("camp perf")
        .expect("present");
    assert_eq!(perf.campaign_id, campaign);
    assert_eq!(perf.link_count, 2);
    assert_eq!(perf.total_clicks, 2);

    let links = svc
        .get_links_by_campaign(&campaign)
        .await
        .expect("links by campaign");
    assert_eq!(links.len(), 2);

    cleanup(&pool, &link_a, None).await;
    cleanup(&pool, &link_b, None).await;
}

#[tokio::test]
async fn journey_map_and_source_content_listing() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let source = SourceId::new(format!("an-journey-{}", Uuid::new_v4()));
    let content_id = seed_content(&pool, &source).await;
    let campaign = CampaignId::new(format!("camp-{}", Uuid::new_v4()));
    let link_id = seed_link(&pool, &campaign, Some(content_id.clone())).await;
    let svc = LinkAnalyticsService::new(&pool).expect("svc");

    svc.track_click(&track_params(
        &link_id,
        &SessionId::new(format!("s-{}", Uuid::new_v4())),
    ))
    .await
    .expect("click");

    let by_source = svc
        .get_links_by_source_content(&content_id)
        .await
        .expect("links by source content");
    assert_eq!(by_source.len(), 1);
    assert_eq!(by_source[0].id, link_id);

    let journey = svc
        .get_content_journey_map(Some(100), Some(0))
        .await
        .expect("journey");
    assert!(
        journey
            .iter()
            .any(|n| n.source_content_id == content_id && n.click_count >= 1),
        "expected the clicked link to appear in the journey map"
    );

    cleanup(&pool, &link_id, Some(&source)).await;
}

#[tokio::test]
async fn performance_for_missing_link_is_none() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let svc = LinkAnalyticsService::new(&pool).expect("svc");

    let missing = LinkId::new(format!("missing-{}", Uuid::new_v4()));
    assert!(
        svc.get_link_performance(&missing)
            .await
            .expect("query")
            .is_none()
    );

    let missing_campaign = CampaignId::new(format!("missing-c-{}", Uuid::new_v4()));
    assert!(
        svc.get_campaign_performance(&missing_campaign)
            .await
            .expect("query")
            .is_none()
    );
}
