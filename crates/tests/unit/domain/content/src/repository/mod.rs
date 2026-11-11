//! Tests for content repository mutations.

mod mutations;

#[tokio::test]
async fn catalog_stats_use_primary_with_an_unavailable_replica() {
    use std::sync::Arc;
    use systemprompt_content::models::CreateContentParams;
    use systemprompt_content::repository::ContentRepository;
    use systemprompt_database::Database;
    use systemprompt_identifiers::SourceId;
    use systemprompt_test_fixtures::test_db_pool;
    use systemprompt_traits::ContentCatalogStats;
    use uuid::Uuid;

    let db = test_db_pool().await;
    let source = SourceId::new(format!("replica-{}", Uuid::new_v4()));
    let primary = ContentRepository::new(&db);
    primary
        .create(&CreateContentParams::new(
            format!("r-{}", Uuid::new_v4().simple()),
            "Public".to_owned(),
            "desc".to_owned(),
            "body".to_owned(),
            source.clone(),
        ))
        .await
        .expect("seed public page");
    let replica =
        sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed").unwrap();
    replica.close().await;
    let split = Arc::new(Database::from_pools(
        Arc::new(replica),
        Some(db.write_pool()),
    ));
    let repository = ContentRepository::new(&split);
    let public_pages = repository.count_public_pages().await;
    let listed = repository.list(1, 0).await;
    primary.delete_by_source(&source).await.expect("cleanup");
    assert!(
        public_pages.expect("primary count") >= 1,
        "the seeded public page is counted through the primary"
    );
    assert!(listed.is_err());
}
