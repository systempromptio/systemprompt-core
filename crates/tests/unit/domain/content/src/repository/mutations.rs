//! DB-backed behavioral tests for [`ContentRepository`] update resolution.
//!
//! Focuses on `ResolvedUpdate`: a `Set` category is applied while unspecified
//! fields (kind here) fall back to the current row. Also asserts typed error
//! propagation when the pool is closed.

use systemprompt_content::models::{CreateContentParams, UpdateContentParams};
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
};
use uuid::Uuid;

async fn cleanup(pool: &DbPool, source: &SourceId) {
    ContentRepository::new(pool)
        .expect("repo")
        .delete_by_source(source)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn update_sets_category_and_preserves_unspecified_kind() {
    let Ok(url) = fixture_database_url() else { return };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ContentRepository::new(&pool).expect("repo");
    let source = SourceId::new(format!("mut-{}", Uuid::new_v4()));
    let slug = format!("m-{}", Uuid::new_v4().simple());

    let created = repo
        .create(
            &CreateContentParams::new(
                slug.clone(),
                "Original".to_owned(),
                "desc".to_owned(),
                "body".to_owned(),
                source.clone(),
            )
            .with_kind("guide".to_owned())
            .with_category_id(Some(CategoryId::new("old-cat"))),
        )
        .await
        .expect("create");

    let new_category = CategoryId::new("new-cat");
    let updated = repo
        .update(
            &UpdateContentParams::new(
                created.id.clone(),
                "Updated Title".to_owned(),
                "new desc".to_owned(),
                "new body".to_owned(),
            )
            .with_category_id(Some(Some(new_category.clone())))
            .with_version_hash("h2".to_owned()),
        )
        .await
        .expect("update");

    assert_eq!(updated.title, "Updated Title");
    assert_eq!(updated.category_id.as_ref().map(CategoryId::as_str), Some("new-cat"));
    // kind was not specified on the update, so the current row's value stands.
    assert_eq!(updated.kind, "guide");

    cleanup(&pool, &source).await;
}

#[tokio::test]
async fn update_on_closed_pool_propagates_error() {
    let pool = closed_db_pool().await;
    let repo = ContentRepository::new(&pool).expect("repo");

    let result = repo
        .update(
            &UpdateContentParams::new(
                systemprompt_identifiers::ContentId::new("nope"),
                "t".to_owned(),
                "d".to_owned(),
                "b".to_owned(),
            ),
        )
        .await;

    assert!(result.is_err(), "closed pool must fail the update");
}
