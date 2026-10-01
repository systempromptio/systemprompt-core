//! Tests for content repository mutations.

mod mutations;

#[tokio::test]
async fn catalog_stats_use_primary_with_an_unavailable_replica() {
    use std::sync::Arc;
    use systemprompt_content::repository::ContentRepository;
    use systemprompt_database::Database;
    use systemprompt_test_fixtures::test_db_pool;
    use systemprompt_traits::ContentCatalogStats;

    let db = test_db_pool().await;
    let replica =
        sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed").unwrap();
    replica.close().await;
    let split = Arc::new(Database::from_pools(
        Arc::new(replica),
        Some(db.write_pool_arc().unwrap()),
    ));
    let repository = ContentRepository::new(&split).unwrap();
    assert!(
        repository
            .count_public_pages()
            .await
            .expect("primary count")
            >= 0
    );
    assert!(repository.list(1, 0).await.is_err());
}
