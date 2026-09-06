//! DB-backed tests for [`McpProxyIdentityRepository`].

use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_mcp::repository::{McpProxyIdentityRepository, ProxyIdentityRow};
use systemprompt_models::auth::{Permission, UserType};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn session(prefix: &str) -> SessionId {
    SessionId::new(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()))
}

fn row(token: &str) -> ProxyIdentityRow {
    ProxyIdentityRow {
        user_id: UserId::new(uuid::Uuid::new_v4().to_string()),
        user_type: UserType::Admin,
        permissions: vec![Permission::Admin, Permission::HookGovern],
        auth_token: JwtToken::new(token),
    }
}

async fn expire(db: &systemprompt_database::DbPool, id: &SessionId) {
    let write = db.write_pool_arc().unwrap();
    sqlx::query("UPDATE mcp_proxy_identities SET expires_at = NOW() - INTERVAL '1 hour' WHERE session_id = $1")
        .bind(id.as_str())
        .execute(write.as_ref())
        .await
        .unwrap();
}

#[tokio::test]
async fn upsert_then_find_round_trips_the_identity() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpProxyIdentityRepository::new(&db).unwrap();
    let id = session("pid-rt");
    let identity = row("tok-1");

    repo.upsert(&id, &identity).await.unwrap();
    let found = repo.find(&id).await.unwrap().expect("stored identity");

    assert_eq!(found.user_id, identity.user_id);
    assert_eq!(found.user_type, UserType::Admin);
    assert_eq!(found.permissions, identity.permissions);
    assert_eq!(found.auth_token.as_str(), "tok-1");
}

#[tokio::test]
async fn upsert_replaces_the_identity_and_refreshes_expiry() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpProxyIdentityRepository::new(&db).unwrap();
    let id = session("pid-up");

    repo.upsert(&id, &row("tok-old")).await.unwrap();
    expire(&db, &id).await;
    assert!(repo.find(&id).await.unwrap().is_none());

    let replacement = row("tok-new");
    repo.upsert(&id, &replacement).await.unwrap();
    let found = repo.find(&id).await.unwrap().expect("refreshed identity");
    assert_eq!(found.auth_token.as_str(), "tok-new");
    assert_eq!(found.user_id, replacement.user_id);
}

#[tokio::test]
async fn find_unknown_session_returns_none() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpProxyIdentityRepository::new(&db).unwrap();
    assert!(repo.find(&session("pid-none")).await.unwrap().is_none());
}

#[tokio::test]
async fn expired_identity_is_not_found() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpProxyIdentityRepository::new(&db).unwrap();
    let id = session("pid-exp");

    repo.upsert(&id, &row("tok")).await.unwrap();
    expire(&db, &id).await;

    assert!(repo.find(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn delete_removes_the_identity() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpProxyIdentityRepository::new(&db).unwrap();
    let id = session("pid-del");

    repo.upsert(&id, &row("tok")).await.unwrap();
    repo.delete(&id).await.unwrap();

    assert!(repo.find(&id).await.unwrap().is_none());
    repo.delete(&id).await.unwrap();
}

#[tokio::test]
async fn cleanup_expired_counts_only_expired_rows() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpProxyIdentityRepository::new(&db).unwrap();
    let live = session("pid-live");
    let stale = session("pid-stale");

    repo.upsert(&live, &row("tok")).await.unwrap();
    repo.upsert(&stale, &row("tok")).await.unwrap();
    expire(&db, &stale).await;

    let removed = repo.cleanup_expired().await.unwrap();
    assert!(
        removed >= 1,
        "expected at least the stale row, got {removed}"
    );
    assert!(repo.find(&live).await.unwrap().is_some());
    assert!(repo.find(&stale).await.unwrap().is_none());

    repo.delete(&live).await.unwrap();
}
