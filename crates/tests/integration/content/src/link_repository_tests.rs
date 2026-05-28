//! DB-backed tests for `LinkRepository` — exercises link create / lookup /
//! list / delete round-trips against a live Postgres schema.

use chrono::Utc;
use systemprompt_content::models::CreateLinkParams;
use systemprompt_content::repository::link::LinkRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::LinkId;

async fn try_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

fn unique_short_code() -> String {
    format!("c{}", uuid::Uuid::new_v4().simple())
}

fn sample_params(short_code: String) -> CreateLinkParams {
    CreateLinkParams::new(
        short_code,
        "https://example.com/target".to_owned(),
        "redirect".to_owned(),
    )
    .with_campaign_name(Some("integration".to_owned()))
    .with_link_text(Some("Try systemprompt".to_owned()))
    .with_destination_type(Some("external".to_owned()))
    .with_is_active(true)
    .with_expires_at(Some(Utc::now() + chrono::Duration::days(30)))
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = try_db().await else {
        return;
    };
    assert!(LinkRepository::new(&db).is_ok());
}

#[tokio::test]
async fn create_then_get_link_by_short_code() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = LinkRepository::new(&db).expect("repo");
    let short_code = unique_short_code();
    let params = sample_params(short_code.clone());

    let created = repo.create_link(&params).await.expect("create link");
    assert_eq!(created.short_code, short_code);
    assert_eq!(created.target_url, params.target_url);
    assert_eq!(created.link_type, "redirect");

    let fetched = repo
        .get_link_by_short_code(&short_code)
        .await
        .expect("query")
        .expect("present");
    assert_eq!(fetched.id, created.id);

    repo.delete_link(&created.id).await.ok();
}

#[tokio::test]
async fn get_link_by_short_code_returns_none_for_unknown() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = LinkRepository::new(&db).expect("repo");
    let res = repo
        .get_link_by_short_code("nope-no-such-code")
        .await
        .expect("query");
    assert!(res.is_none());
}

#[tokio::test]
async fn get_link_by_id_returns_none_for_unknown_id() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = LinkRepository::new(&db).expect("repo");
    let missing = LinkId::generate();
    let res = repo.get_link_by_id(&missing).await.expect("query");
    assert!(res.is_none());
}

#[tokio::test]
async fn delete_link_returns_true_for_existing_false_for_missing() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = LinkRepository::new(&db).expect("repo");
    let short = unique_short_code();
    let created = repo
        .create_link(&sample_params(short.clone()))
        .await
        .expect("create");

    let deleted = repo.delete_link(&created.id).await.expect("delete");
    assert!(deleted, "first delete should return true");

    let again = repo.delete_link(&created.id).await.expect("delete twice");
    assert!(!again, "second delete should return false");
}

#[tokio::test]
async fn list_links_by_campaign_filters_by_campaign_id() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = LinkRepository::new(&db).expect("repo");
    let campaign = systemprompt_identifiers::CampaignId::new(uuid::Uuid::new_v4().to_string());
    let short = unique_short_code();
    let params = sample_params(short.clone()).with_campaign_id(Some(campaign.clone()));
    let created = repo.create_link(&params).await.expect("create");

    let listed = repo
        .list_links_by_campaign(&campaign)
        .await
        .expect("list by campaign");
    assert!(
        listed.iter().any(|l| l.id == created.id),
        "listing must contain the link we just created"
    );

    repo.delete_link(&created.id).await.ok();
}

#[tokio::test]
async fn list_links_by_source_content_filters_correctly() {
    use systemprompt_content::models::CreateContentParams;
    use systemprompt_content::repository::ContentRepository;
    use systemprompt_identifiers::{LocaleCode, SourceId};

    let Some(db) = try_db().await else {
        return;
    };
    let content_repo = ContentRepository::new(&db).expect("content repo");
    let source_id = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let slug = format!("link-src-{}", uuid::Uuid::new_v4().simple());
    let content_params = CreateContentParams {
        slug,
        locale: LocaleCode::new("en"),
        title: "Source".to_owned(),
        description: "for link FK".to_owned(),
        body: "body".to_owned(),
        author: "Author".to_owned(),
        published_at: chrono::Utc::now(),
        keywords: String::new(),
        kind: "article".to_owned(),
        image: None,
        category_id: None,
        source_id: source_id.clone(),
        version_hash: "h".to_owned(),
        links: serde_json::json!([]),
        public: true,
    };
    let content = content_repo
        .create(&content_params)
        .await
        .expect("create content");

    let repo = LinkRepository::new(&db).expect("repo");
    let params =
        sample_params(unique_short_code()).with_source_content_id(Some(content.id.clone()));
    let created = repo.create_link(&params).await.expect("create link");

    let listed = repo
        .list_links_by_source_content(&content.id)
        .await
        .expect("list by source_content");
    assert!(listed.iter().any(|l| l.id == created.id));

    repo.delete_link(&created.id).await.ok();
    content_repo.delete(&content.id).await.ok();
}

#[tokio::test]
async fn find_link_by_source_and_target_matches_active_link() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = LinkRepository::new(&db).expect("repo");
    let source_page = format!("/page-{}", uuid::Uuid::new_v4().simple());
    let target = format!("https://example.com/{}", uuid::Uuid::new_v4());
    let params = CreateLinkParams::new(unique_short_code(), target.clone(), "redirect".to_owned())
        .with_source_page(Some(source_page.clone()))
        .with_is_active(true);
    let created = repo.create_link(&params).await.expect("create");

    let found = repo
        .find_link_by_source_and_target(&source_page, &target)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.id, created.id);

    repo.delete_link(&created.id).await.ok();
}
