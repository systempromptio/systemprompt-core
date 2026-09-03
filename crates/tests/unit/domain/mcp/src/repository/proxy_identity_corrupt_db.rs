//! Decode-failure and hard-delete behaviour of [`McpProxyIdentityRepository`].
//!
//! `find` filters on `expires_at > NOW()`, so it cannot witness a deletion:
//! the expiry tests here count rows with SQL instead.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_mcp::repository::McpProxyIdentityRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool() -> (DbPool, Arc<PgPool>) {
    let url = fixture_database_url().expect("DATABASE_URL must be set");
    let db = fixture_db_pool(&url).await.expect("pool");
    let write = db.write_pool_arc().expect("write pool");
    (db, write)
}

fn session(prefix: &str) -> SessionId {
    SessionId::new(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()))
}

async fn insert_raw(
    write: &PgPool,
    id: &SessionId,
    user_type: &str,
    permissions: serde_json::Value,
    expires_in: &str,
) {
    sqlx::query(
        "INSERT INTO mcp_proxy_identities \
         (session_id, user_id, user_type, permissions, auth_token, expires_at) \
         VALUES ($1, $2, $3, $4, 'tok', NOW() + $5::interval)",
    )
    .bind(id.as_str())
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user_type)
    .bind(permissions)
    .bind(expires_in)
    .execute(write)
    .await
    .expect("insert raw identity");
}

async fn row_count(write: &PgPool, id: &SessionId) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mcp_proxy_identities WHERE session_id = $1")
        .bind(id.as_str())
        .fetch_one(write)
        .await
        .expect("count rows")
}

#[tokio::test]
async fn find_rejects_a_row_whose_user_type_is_not_a_known_variant() {
    let (db, write) = pool().await;
    let repo = McpProxyIdentityRepository::new(&db).expect("repo");
    let id = session("pid-badtype");
    insert_raw(&write, &id, "sovereign", serde_json::json!([]), "1 hour").await;

    let err = repo
        .find(&id)
        .await
        .expect_err("an unknown user_type must not silently resolve an identity");
    assert!(
        format!("{err:?}").contains("Validation"),
        "expected a validation error, got {err:?}"
    );

    repo.delete(&id).await.expect("delete");
}

#[tokio::test]
async fn find_rejects_a_row_whose_permissions_are_not_a_permission_list() {
    let (db, write) = pool().await;
    let repo = McpProxyIdentityRepository::new(&db).expect("repo");
    let id = session("pid-badperms");
    insert_raw(
        &write,
        &id,
        "admin",
        serde_json::json!({"admin": true}),
        "1 hour",
    )
    .await;

    assert!(
        repo.find(&id).await.is_err(),
        "a non-list permissions column must fail loudly, never decode to an empty grant set"
    );

    repo.delete(&id).await.expect("delete");
}

#[tokio::test]
async fn delete_removes_the_row_even_once_it_has_expired() {
    let (db, write) = pool().await;
    let repo = McpProxyIdentityRepository::new(&db).expect("repo");
    let id = session("pid-delexp");
    insert_raw(&write, &id, "admin", serde_json::json!([]), "-1 hour").await;
    assert_eq!(row_count(&write, &id).await, 1);

    repo.delete(&id).await.expect("delete");

    assert_eq!(
        row_count(&write, &id).await,
        0,
        "the row must be gone from the table, not merely invisible to find()"
    );
}

#[tokio::test]
async fn cleanup_expired_physically_removes_expired_rows_and_spares_live_ones() {
    let (db, write) = pool().await;
    let repo = McpProxyIdentityRepository::new(&db).expect("repo");
    let live = session("pid-cl-live");
    let stale = session("pid-cl-stale");
    insert_raw(&write, &live, "admin", serde_json::json!([]), "1 hour").await;
    insert_raw(&write, &stale, "admin", serde_json::json!([]), "-1 hour").await;

    repo.cleanup_expired().await.expect("cleanup");

    assert_eq!(
        row_count(&write, &stale).await,
        0,
        "cleanup must delete the expired row; find() would report None either way"
    );
    assert_eq!(row_count(&write, &live).await, 1);

    repo.delete(&live).await.expect("delete");
}
