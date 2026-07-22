//! DB-backed tests for `CleanupRepository`.
//!
//! Each test seeds uniquely-keyed rows and asserts only on those rows — the
//! cleanup DELETEs are table-wide, so returned counts are checked as lower
//! bounds and the definitive assertion is that the seeded row is gone.

use chrono::{Duration, Utc};
use systemprompt_database::CleanupRepository;

use crate::services::db_helper::pool;

async fn repo_and_pool() -> Option<(CleanupRepository, sqlx::PgPool)> {
    let db = pool().await?;
    let pg = db.write_pool_arc().ok()?;
    Some((CleanupRepository::new((*pg).clone()), (*pg).clone()))
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn insert_log(pool: &sqlx::PgPool, id: &str, user_id: Option<&str>, age_days: i64) {
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id) VALUES ($1, $2, \
         'INFO', 'cleanup-test', 'cleanup fixture', $3)",
    )
    .bind(id)
    .bind(Utc::now() - Duration::days(age_days))
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert log fixture");
}

async fn log_exists(pool: &sqlx::PgPool, id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM logs WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("log existence probe")
}

#[tokio::test]
async fn delete_old_logs_removes_rows_past_cutoff_and_keeps_recent() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let old_id = unique("old_log");
    let fresh_id = unique("fresh_log");
    insert_log(&pg, &old_id, None, 4000).await;
    insert_log(&pg, &fresh_id, None, 0).await;

    let counted = repo.count_old_logs(3650).await.expect("count old");
    assert!(counted >= 1);

    let deleted = repo.delete_old_logs(3650).await.expect("delete old");
    assert!(deleted >= 1);

    assert!(!log_exists(&pg, &old_id).await);
    assert!(log_exists(&pg, &fresh_id).await);

    let _ = sqlx::query("DELETE FROM logs WHERE id = $1")
        .bind(&fresh_id)
        .execute(&pg)
        .await;
}

#[tokio::test]
async fn delete_orphaned_logs_removes_rows_for_missing_users() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let orphan_id = unique("orphan_log");
    let ghost_user = unique("ghost_user");
    insert_log(&pg, &orphan_id, Some(&ghost_user), 0).await;

    let deleted = repo.delete_orphaned_logs().await.expect("delete orphaned");
    assert!(deleted >= 1);
    assert!(!log_exists(&pg, &orphan_id).await);
}

#[tokio::test]
async fn delete_orphaned_mcp_executions_removes_rows_without_context() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let exec_id = unique("mcp_exec");
    let ghost_context = unique("ghost_ctx");
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, \
         input, status, user_id, context_id) VALUES ($1, 'tool', 'server', NOW(), '{}', \
         'pending', 'cleanup-test-user', $2)",
    )
    .bind(&exec_id)
    .bind(&ghost_context)
    .execute(&pg)
    .await
    .expect("insert mcp execution fixture");

    let deleted = repo
        .delete_orphaned_mcp_executions()
        .await
        .expect("delete orphaned executions");
    assert!(deleted >= 1);

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1)",
    )
    .bind(&exec_id)
    .fetch_one(&pg)
    .await
    .expect("execution existence probe");
    assert!(!exists);
}

async fn seed_user_and_client(pool: &sqlx::PgPool) -> (String, String) {
    let user_id = unique("cleanup_user");
    let client_id = unique("cleanup_client");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&user_id)
        .bind(format!("{user_id}@cleanup.test"))
        .execute(pool)
        .await
        .expect("insert user fixture");
    sqlx::query(
        "INSERT INTO oauth_clients (client_id, client_name, owner_user_id) VALUES ($1, $1, $2)",
    )
    .bind(&client_id)
    .bind(&user_id)
    .execute(pool)
    .await
    .expect("insert client fixture");
    (user_id, client_id)
}

async fn remove_user_and_client(pool: &sqlx::PgPool, user_id: &str, client_id: &str) {
    let _ = sqlx::query("DELETE FROM oauth_clients WHERE client_id = $1")
        .bind(client_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn delete_expired_oauth_tokens_removes_expired_and_keeps_live() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let (user_id, client_id) = seed_user_and_client(&pg).await;
    let expired = unique("expired_token");
    let live = unique("live_token");
    for (token, offset_hours) in [(&expired, -1_i64), (&live, 24)] {
        sqlx::query(
            "INSERT INTO oauth_refresh_tokens (token_id, client_id, user_id, scope, expires_at, \
             family_id) VALUES ($1, $2, $3, 'openid', $4, $1)",
        )
        .bind(token)
        .bind(&client_id)
        .bind(&user_id)
        .bind(Utc::now() + Duration::hours(offset_hours))
        .execute(&pg)
        .await
        .expect("insert token fixture");
    }

    let deleted = repo
        .delete_expired_oauth_tokens()
        .await
        .expect("delete expired tokens");
    assert!(deleted >= 1);

    let remaining: Vec<String> =
        sqlx::query_scalar("SELECT token_id FROM oauth_refresh_tokens WHERE token_id IN ($1, $2)")
            .bind(&expired)
            .bind(&live)
            .fetch_all(&pg)
            .await
            .expect("remaining tokens");
    assert_eq!(remaining, vec![live]);

    remove_user_and_client(&pg, &user_id, &client_id).await;
}

#[tokio::test]
async fn delete_expired_oauth_codes_removes_used_and_expired_codes() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let (user_id, client_id) = seed_user_and_client(&pg).await;
    let used = unique("used_code");
    let fresh = unique("fresh_code");
    for (code, used_at, offset_hours) in [(&used, Some(Utc::now()), 24_i64), (&fresh, None, 24)] {
        sqlx::query(
            "INSERT INTO oauth_auth_codes (code, client_id, user_id, redirect_uri, scope, \
             expires_at, used_at) VALUES ($1, $2, $3, 'https://cb.test', 'openid', $4, $5)",
        )
        .bind(code)
        .bind(&client_id)
        .bind(&user_id)
        .bind(Utc::now() + Duration::hours(offset_hours))
        .bind(used_at)
        .execute(&pg)
        .await
        .expect("insert code fixture");
    }

    let deleted = repo
        .delete_expired_oauth_codes()
        .await
        .expect("delete expired codes");
    assert!(deleted >= 1);

    let remaining: Vec<String> =
        sqlx::query_scalar("SELECT code FROM oauth_auth_codes WHERE code IN ($1, $2)")
            .bind(&used)
            .bind(&fresh)
            .fetch_all(&pg)
            .await
            .expect("remaining codes");
    assert_eq!(remaining, vec![fresh]);

    remove_user_and_client(&pg, &user_id, &client_id).await;
}

#[tokio::test]
async fn delete_expired_oauth_state_bindings_removes_expired_rows() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let hash = unique("state_hash");
    sqlx::query(
        "INSERT INTO oauth_state_bindings (state_token_hash, return_to, client_id, redirect_uri, \
         expires_at) VALUES ($1, '/', 'cleanup-client', 'https://cb.test', NOW() - INTERVAL '1 \
         hour')",
    )
    .bind(&hash)
    .execute(&pg)
    .await
    .expect("insert state binding fixture");

    let deleted = repo
        .delete_expired_oauth_state_bindings()
        .await
        .expect("delete expired bindings");
    assert!(deleted >= 1);

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM oauth_state_bindings WHERE state_token_hash = $1)",
    )
    .bind(&hash)
    .fetch_one(&pg)
    .await
    .expect("binding existence probe");
    assert!(!exists);
}

#[tokio::test]
async fn delete_expired_oauth_jti_revocations_removes_expired_rows() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let jti = unique("jti");
    sqlx::query(
        "INSERT INTO oauth_jti_revocations (jti, user_id, exp) VALUES ($1, $2, NOW() - INTERVAL \
         '1 hour')",
    )
    .bind(&jti)
    .bind(uuid::Uuid::new_v4())
    .execute(&pg)
    .await
    .expect("insert revocation fixture");

    let deleted = repo
        .delete_expired_oauth_jti_revocations()
        .await
        .expect("delete expired revocations");
    assert!(deleted >= 1);

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM oauth_jti_revocations WHERE jti = $1)",
    )
    .bind(&jti)
    .fetch_one(&pg)
    .await
    .expect("revocation existence probe");
    assert!(!exists);
}

#[tokio::test]
async fn delete_expired_id_jag_replays_removes_expired_rows() {
    let Some((repo, pg)) = repo_and_pool().await else {
        return;
    };
    let jti = unique("replay_jti");
    sqlx::query(
        "INSERT INTO id_jag_replay (jti, expires_at) VALUES ($1, NOW() - INTERVAL '1 hour')",
    )
    .bind(&jti)
    .execute(&pg)
    .await
    .expect("insert replay fixture");

    let deleted = repo
        .delete_expired_id_jag_replays()
        .await
        .expect("delete expired replays");
    assert!(deleted >= 1);

    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM id_jag_replay WHERE jti = $1)")
            .bind(&jti)
            .fetch_one(&pg)
            .await
            .expect("replay existence probe");
    assert!(!exists);
}
