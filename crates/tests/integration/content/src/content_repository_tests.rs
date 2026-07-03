//! DB-backed tests for `ContentRepository`: create / get / list / update /
//! delete round-trips against a real Postgres schema. These exercise the
//! `sqlx::query_as!` macros in `repository/content/{mutations,queries}.rs`
//! which the pure-unit tests cannot reach.

use chrono::Utc;
use systemprompt_content::models::{CategoryIdUpdate, CreateContentParams, UpdateContentParams};
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, LocaleCode, SourceId};

async fn try_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

fn unique_source_id() -> SourceId {
    SourceId::new(format!("src-{}", uuid::Uuid::new_v4()))
}

fn unique_slug() -> String {
    format!("slug-{}", uuid::Uuid::new_v4())
}

fn sample_params(source_id: SourceId, slug: String) -> CreateContentParams {
    CreateContentParams {
        slug,
        locale: LocaleCode::new("en"),
        title: "Sample title".to_owned(),
        description: "Sample description".to_owned(),
        body: "Sample body".to_owned(),
        author: "Test Author".to_owned(),
        published_at: Utc::now(),
        keywords: "kw1, kw2".to_owned(),
        kind: "article".to_owned(),
        image: Some("/img.png".to_owned()),
        category_id: None,
        source_id,
        version_hash: "deadbeef".to_owned(),
        links: serde_json::json!([]),
        public: true,
    }
}

#[tokio::test]
async fn repository_new_succeeds_against_real_pool() {
    let Some(db) = try_db().await else {
        eprintln!("Skipping (DATABASE_URL not set)");
        return;
    };
    drop(ContentRepository::new(&db).expect("ContentRepository::new should succeed"));
}

#[tokio::test]
async fn create_then_get_by_id_round_trips() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let slug = unique_slug();
    let params = sample_params(source.clone(), slug.clone());

    let created = repo.create(&params).await.expect("create content");
    assert_eq!(created.slug, slug);
    assert_eq!(created.title, "Sample title");
    assert!(created.public);

    let fetched = repo
        .get_by_id(&created.id)
        .await
        .expect("get_by_id")
        .expect("content row");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, slug);
    assert_eq!(fetched.author, "Test Author");

    repo.delete(&created.id).await.expect("cleanup delete");
}

#[tokio::test]
async fn get_by_slug_and_locale_finds_existing_row() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let slug = unique_slug();
    let params = sample_params(source.clone(), slug.clone());
    let created = repo.create(&params).await.expect("create");

    let locale = LocaleCode::new("en");
    let fetched = repo
        .get_by_slug(&slug, &locale)
        .await
        .expect("query")
        .expect("row");
    assert_eq!(fetched.id, created.id);

    let by_src = repo
        .get_by_source_and_slug(&source, &slug, &locale)
        .await
        .expect("query by source+slug")
        .expect("row");
    assert_eq!(by_src.id, created.id);

    repo.delete(&created.id).await.expect("cleanup");
}

#[tokio::test]
async fn get_by_id_returns_none_for_unknown_id() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let missing = ContentId::new(uuid::Uuid::new_v4().to_string());
    let result = repo.get_by_id(&missing).await.expect("query unknown id");
    assert!(result.is_none());
}

#[tokio::test]
async fn list_by_source_returns_inserted_rows() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let slug_a = unique_slug();
    let slug_b = unique_slug();

    let a = repo
        .create(&sample_params(source.clone(), slug_a.clone()))
        .await
        .expect("create a");
    let b = repo
        .create(&sample_params(source.clone(), slug_b.clone()))
        .await
        .expect("create b");

    let locale = LocaleCode::new("en");
    let rows = repo
        .list_by_source(&source, &locale)
        .await
        .expect("list_by_source");
    let slugs: Vec<&str> = rows.iter().map(|c| c.slug.as_str()).collect();
    assert!(slugs.contains(&slug_a.as_str()));
    assert!(slugs.contains(&slug_b.as_str()));

    let limited = repo
        .list_by_source_limited(&source, &locale, 1)
        .await
        .expect("limited");
    assert_eq!(limited.len(), 1);

    repo.delete(&a.id).await.ok();
    repo.delete(&b.id).await.ok();
}

#[tokio::test]
async fn list_paginates_with_limit_and_offset() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let first_page = repo.list(5, 0).await.expect("list page");
    assert!(first_page.len() <= 5);

    let all_page = repo.list_all(10, 0).await.expect("list_all");
    assert!(all_page.len() <= 10);
}

#[tokio::test]
async fn category_exists_returns_false_for_unknown_category() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let missing = systemprompt_identifiers::CategoryId::new(uuid::Uuid::new_v4().to_string());
    let exists = repo.category_exists(&missing).await.expect("query");
    assert!(!exists, "fresh-UUID category must not exist");
}

#[tokio::test]
async fn update_changes_title_and_description() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let slug = unique_slug();
    let created = repo
        .create(&sample_params(source.clone(), slug.clone()))
        .await
        .expect("create");

    let update = UpdateContentParams::new(
        created.id.clone(),
        "Updated Title".to_owned(),
        "Updated desc".to_owned(),
        "Updated body".to_owned(),
    )
    .with_keywords("new, kws".to_owned())
    .with_image(None)
    .with_version_hash("newhash".to_owned())
    .with_category_id(CategoryIdUpdate::Clear)
    .with_public(Some(false));

    let updated = repo.update(&update).await.expect("update");
    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.description, "Updated desc");
    assert!(!updated.public);
    assert!(updated.image.is_none());

    repo.delete(&created.id).await.ok();
}

#[tokio::test]
async fn delete_by_source_removes_all_rows_for_source() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let _a = repo
        .create(&sample_params(source.clone(), unique_slug()))
        .await
        .expect("a");
    let _b = repo
        .create(&sample_params(source.clone(), unique_slug()))
        .await
        .expect("b");
    let _c = repo
        .create(&sample_params(source.clone(), unique_slug()))
        .await
        .expect("c");

    let deleted = repo
        .delete_by_source(&source)
        .await
        .expect("delete_by_source");
    assert!(deleted >= 3, "expected >=3 rows deleted, got {deleted}");

    let leftover = repo
        .list_by_source(&source, &LocaleCode::new("en"))
        .await
        .expect("post-delete list");
    assert!(leftover.is_empty());
}

#[tokio::test]
async fn find_sources_by_slug_returns_distinct_sources() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let slug = unique_slug();
    let source_a = unique_source_id();
    let source_b = unique_source_id();
    let _ = repo
        .create(&sample_params(source_a.clone(), slug.clone()))
        .await
        .expect("a");
    let row_b = sample_params(source_b.clone(), slug.clone());
    let b_locale = LocaleCode::new("fr");
    let mut row_b = row_b;
    row_b.locale = b_locale.clone();
    let created_b = repo.create(&row_b).await.expect("b in fr locale");
    assert_eq!(
        created_b.locale, b_locale,
        "created row keeps the fr locale"
    );

    let sources_en = repo
        .find_sources_by_slug(&slug, &LocaleCode::new("en"))
        .await
        .expect("find by slug en");
    assert!(sources_en.iter().any(|s| s == &source_a));
    assert!(
        !sources_en.iter().any(|s| s == &source_b),
        "fr-only source must not surface under the en locale"
    );

    let sources_fr = repo
        .find_sources_by_slug(&slug, &b_locale)
        .await
        .expect("find by slug fr");
    assert!(sources_fr.iter().any(|s| s == &source_b));

    repo.delete_by_source(&source_a).await.ok();
    repo.delete_by_source(&source_b).await.ok();
}

#[tokio::test]
async fn list_slugs_with_locales_by_source_lists_inserted() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let slug = unique_slug();
    let _ = repo
        .create(&sample_params(source.clone(), slug.clone()))
        .await
        .expect("create");

    let pairs = repo
        .list_slugs_with_locales_by_source(&source)
        .await
        .expect("list slug/locale pairs");
    assert!(
        pairs.iter().any(|(s, l)| s == &slug && l.as_str() == "en"),
        "should contain ({slug}, en); got {pairs:?}"
    );

    repo.delete_by_source(&source).await.ok();
}

#[tokio::test]
async fn get_popular_content_ids_runs_without_error_when_no_metrics() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = ContentRepository::new(&db).expect("repo");
    let source = unique_source_id();
    let ids = repo
        .get_popular_content_ids(&source, 30, 5)
        .await
        .expect("popular");
    assert!(ids.is_empty());
}
