//! DB-backed tests for `SearchRepository`.

use systemprompt_content::repository::SearchRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::CategoryId;

async fn try_db() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn search_repository_new_succeeds() {
    let Some(db) = try_db().await else {
        return;
    };
    assert!(SearchRepository::new(&db).is_ok());
}

#[tokio::test]
async fn search_by_unknown_category_returns_empty() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = SearchRepository::new(&db).expect("repo");
    let cat = CategoryId::new(uuid::Uuid::new_v4().to_string());
    let results = repo
        .search_by_category(&cat, 10)
        .await
        .expect("query");
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_by_unknown_keyword_returns_empty_or_unrelated() {
    let Some(db) = try_db().await else {
        return;
    };
    let repo = SearchRepository::new(&db).expect("repo");
    let needle = format!("zzz-{}-zzz", uuid::Uuid::new_v4().simple());
    let results = repo
        .search_by_keyword(&needle, 5)
        .await
        .expect("query");
    assert!(
        results.is_empty(),
        "fresh-UUID needle should match nothing, got {} rows",
        results.len()
    );
}
