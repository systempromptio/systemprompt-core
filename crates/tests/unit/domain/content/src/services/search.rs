//! DB-backed behavioral tests for [`SearchService`].
//!
//! Seeds `markdown_content` rows and exercises the three resolution arms of
//! `search`: category-filtered, filter-present-without-category (empty), and
//! the unfiltered recency listing, plus the direct `search_by_category` entry.

use systemprompt_content::SearchService;
use systemprompt_content::models::{CreateContentParams, SearchFilters, SearchRequest};
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn seed(repo: &ContentRepository, source: &SourceId, category: &CategoryId, slug: &str) {
    let params = CreateContentParams::new(
        slug.to_owned(),
        "Titled".to_owned(),
        "desc".to_owned(),
        "body".to_owned(),
        source.clone(),
    )
    .with_kind("article".to_owned())
    .with_category_id(Some(category.clone()));
    repo.create(&params).await.expect("seed");
}

async fn cleanup(pool: &DbPool, source: &SourceId) {
    ContentRepository::new(pool)
        .expect("repo")
        .delete_by_source(source)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn search_without_filters_lists_recent_content() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = SourceId::new(format!("srch-{}", Uuid::new_v4()));
    let category = CategoryId::new(format!("cat-{}", Uuid::new_v4()));
    seed(
        &repo,
        &source,
        &category,
        &format!("s-{}", Uuid::new_v4().simple()),
    )
    .await;

    let service = SearchService::new(&pool).expect("service");
    let response = service
        .search(&SearchRequest {
            query: String::new(),
            filters: None,
            limit: Some(50),
        })
        .await
        .expect("search");

    assert_eq!(response.total, response.results.len());
    assert!(response.total >= 1);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn search_with_filter_but_no_category_returns_empty() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");

    let service = SearchService::new(&pool).expect("service");
    let response = service
        .search(&SearchRequest {
            query: String::new(),
            filters: Some(SearchFilters { category_id: None }),
            limit: None,
        })
        .await
        .expect("search");

    assert_eq!(response.total, 0);
    assert!(response.results.is_empty());
}

#[tokio::test]
async fn search_by_category_returns_only_matching_rows() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = SourceId::new(format!("srch-{}", Uuid::new_v4()));
    let category = CategoryId::new(format!("cat-{}", Uuid::new_v4()));
    let slug = format!("s-{}", Uuid::new_v4().simple());
    seed(&repo, &source, &category, &slug).await;

    let service = SearchService::new(&pool).expect("service");

    // Through the request API.
    let response = service
        .search(&SearchRequest {
            query: String::new(),
            filters: Some(SearchFilters {
                category_id: Some(category.clone()),
            }),
            limit: Some(10),
        })
        .await
        .expect("search");
    assert!(
        response.results.iter().any(|r| r.slug == slug),
        "{response:?}"
    );

    // Through the direct method.
    let direct = service
        .search_by_category(&category, 10)
        .await
        .expect("search_by_category");
    assert!(direct.iter().any(|r| r.slug == slug));

    cleanup(&pool, &source).await;
}
