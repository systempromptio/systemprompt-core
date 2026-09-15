use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use systemprompt_database::Database;

fn lazy_pool() -> Arc<sqlx::PgPool> {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://u:p@127.0.0.1:5432/x")
        .expect("build lazy pool");
    Arc::new(pool)
}

#[tokio::test]
async fn from_pools_read_only_falls_back_to_read_for_writes() {
    let read = lazy_pool();
    let db = Database::from_pools(Arc::clone(&read), None);

    assert!(!db.has_write_pool());
    assert!(Arc::ptr_eq(&db.pool(), &read));
    assert!(Arc::ptr_eq(&db.write_pool(), &read));
}

#[tokio::test]
async fn from_pools_reuses_distinct_write_pool() {
    let read = lazy_pool();
    let write = lazy_pool();
    let db = Database::from_pools(Arc::clone(&read), Some(Arc::clone(&write)));

    assert!(db.has_write_pool());

    let read_back = db.pool();
    let write_back = db.write_pool();
    assert!(Arc::ptr_eq(&read_back, &read));
    assert!(Arc::ptr_eq(&write_back, &write));
}
