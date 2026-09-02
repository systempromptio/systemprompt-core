//! MCP session lookups resolve on the primary even when the read pool is
//! unusable, so an `initialize` on one replica is visible to the next request
//! on another.

use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::SessionId;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn split_pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    let live = fixture_db_pool(&url).await.ok()?;
    let write = live.write_pool_arc().ok()?;
    let dead = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed").ok()?;
    dead.close().await;
    Some(Arc::new(Database::from_pools(Arc::new(dead), Some(write))))
}

#[tokio::test]
async fn session_lookups_read_the_primary() {
    let Some(db) = split_pool().await else {
        return;
    };
    let repo = McpSessionRepository::new(&db).expect("repo");
    let id = SessionId::new(format!("sess-{}", uuid::Uuid::new_v4().simple()));

    assert!(!repo.exists(&id).await.expect("exists on primary"));
    assert!(repo.find_active(&id).await.expect("find_active on primary").is_none());
    assert!(
        repo.find_initialize_params(&id)
            .await
            .expect("initialize params on primary")
            .is_none()
    );
}
