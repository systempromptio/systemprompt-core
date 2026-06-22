//! DB-backed tests for [`DefaultContentProvider`].
//!
//! Seeds `markdown_content` rows through [`ContentRepository`] and drives the
//! `ContentProvider` trait surface (get by id/slug/source, list, search),
//! asserting the row data round-trips into `ContentItem` / `ContentSummary`.

use systemprompt_content::DefaultContentProvider;
use systemprompt_content::models::CreateContentParams;
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_traits::content::{ContentFilter, ContentProvider};
use uuid::Uuid;

async fn seed(repo: &ContentRepository, source: &SourceId, slug: &str, title: &str) -> ContentId {
    let params = CreateContentParams::new(
        slug.to_owned(),
        title.to_owned(),
        "a description".to_owned(),
        "the body text".to_owned(),
        source.clone(),
    )
    .with_author("Author McTest".to_owned())
    .with_keywords("rust, content".to_owned())
    .with_kind("article".to_owned());
    repo.create(&params).await.expect("seed content").id
}

async fn cleanup(pool: &DbPool, source: &SourceId) {
    let repo = ContentRepository::new(pool).expect("repo");
    repo.delete_by_source(source).await.expect("cleanup");
}

fn unique_source(prefix: &str) -> SourceId {
    SourceId::new(format!("{prefix}-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn get_content_returns_full_item() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source("cp-getid");
    let slug = format!("post-{}", Uuid::new_v4());

    let id = seed(&repo, &source, &slug, "Hello World").await;

    let provider = DefaultContentProvider::new(&pool).expect("provider");
    let item = provider
        .get_content(&id)
        .await
        .expect("get_content")
        .expect("present");

    assert_eq!(item.id, id);
    assert_eq!(item.slug, slug);
    assert_eq!(item.title, "Hello World");
    assert_eq!(item.description, "a description");
    assert_eq!(item.body, "the body text");
    assert_eq!(item.author, "Author McTest");
    assert_eq!(item.kind, "article");
    assert_eq!(item.source_id, source);
    assert!(item.category_id.is_none());

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn get_content_missing_returns_none() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let provider = DefaultContentProvider::new(&pool).expect("provider");

    let missing = ContentId::new(format!("nope-{}", Uuid::new_v4()));
    let result = provider.get_content(&missing).await.expect("get_content");
    assert!(result.is_none());
}

#[tokio::test]
async fn get_content_by_slug_round_trips() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source("cp-slug");
    let slug = format!("by-slug-{}", Uuid::new_v4());

    let id = seed(&repo, &source, &slug, "Slug Title").await;

    let provider = DefaultContentProvider::new(&pool).expect("provider");
    let item = provider
        .get_content_by_slug(&slug)
        .await
        .expect("by slug")
        .expect("present");
    assert_eq!(item.id, id);
    assert_eq!(item.title, "Slug Title");

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn get_content_by_source_and_slug_round_trips() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source("cp-srcslug");
    let slug = format!("src-slug-{}", Uuid::new_v4());

    let id = seed(&repo, &source, &slug, "Source Slug").await;

    let provider = DefaultContentProvider::new(&pool).expect("provider");
    let item = provider
        .get_content_by_source_and_slug(&source, &slug)
        .await
        .expect("by source+slug")
        .expect("present");
    assert_eq!(item.id, id);
    assert_eq!(item.source_id, source);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn list_content_by_source_returns_only_that_source() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source("cp-list");

    let id_a = seed(&repo, &source, &format!("a-{}", Uuid::new_v4()), "A").await;
    let id_b = seed(&repo, &source, &format!("b-{}", Uuid::new_v4()), "B").await;

    let provider = DefaultContentProvider::new(&pool).expect("provider");
    let summaries = provider
        .list_content(ContentFilter {
            source_id: Some(source.clone()),
            ..Default::default()
        })
        .await
        .expect("list");

    assert_eq!(summaries.len(), 2);
    let ids: Vec<_> = summaries.iter().map(|s| s.id.clone()).collect();
    assert!(ids.contains(&id_a));
    assert!(ids.contains(&id_b));
    for s in &summaries {
        assert_eq!(s.source_id, source);
    }

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn list_content_unfiltered_respects_limit() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source("cp-limit");

    seed(&repo, &source, &format!("x-{}", Uuid::new_v4()), "X").await;
    seed(&repo, &source, &format!("y-{}", Uuid::new_v4()), "Y").await;
    seed(&repo, &source, &format!("z-{}", Uuid::new_v4()), "Z").await;

    let provider = DefaultContentProvider::new(&pool).expect("provider");
    let summaries = provider
        .list_content(ContentFilter {
            limit: Some(2),
            offset: Some(0),
            ..Default::default()
        })
        .await
        .expect("list");

    assert_eq!(summaries.len(), 2);

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn search_finds_seeded_keyword() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = unique_source("cp-search");
    let token = format!("zzqq{}", Uuid::new_v4().simple());
    let slug = format!("search-{}", Uuid::new_v4());

    seed(&repo, &source, &slug, &format!("Unique {token} Heading")).await;

    let provider = DefaultContentProvider::new(&pool).expect("provider");
    let results = provider.search(&token, Some(10)).await.expect("search");

    assert!(
        results.iter().any(|r| r.title.contains(&token)),
        "expected search to surface the seeded title, got {results:?}"
    );

    cleanup(&pool, &source).await;
}
