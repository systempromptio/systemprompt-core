//! DB-backed tests for `OauthCleanupRepository`: every expiry sweep removes
//! the seeded expired row and keeps the live one.

use chrono::{Duration, Utc};
use systemprompt_oauth::repository::OauthCleanupRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn repo_and_pool_or_skip() -> Option<(OauthCleanupRepository, sqlx::PgPool)> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    let pg = db.write_pool();
    Some((OauthCleanupRepository::new(&db).ok()?, (*pg).clone()))
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
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
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
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
        .delete_expired_refresh_tokens()
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
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
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
        .delete_expired_auth_codes()
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
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
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
        .delete_expired_state_bindings()
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
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
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
        .delete_expired_jti_revocations()
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
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
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

#[tokio::test]
async fn delete_expired_sweeps_every_table_and_totals_the_counts() {
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
        return;
    };
    let jti = unique("sweep_replay");
    sqlx::query(
        "INSERT INTO id_jag_replay (jti, expires_at) VALUES ($1, NOW() - INTERVAL '1 hour')",
    )
    .bind(&jti)
    .execute(&pg)
    .await
    .expect("insert replay fixture");

    let counts = repo.delete_expired().await.expect("sweep");
    assert!(counts.id_jag_replays >= 1);
    assert!(counts.total() >= counts.id_jag_replays);
}

#[tokio::test]
async fn bridge_exchange_codes_are_swept_once_spent_or_expired() {
    let Some((repo, pg)) = repo_and_pool_or_skip().await else {
        return;
    };
    let (user_id, client_id) = seed_user_and_client(&pg).await;
    let live = unique("code_live");
    let consumed = unique("code_consumed");
    let expired = unique("code_expired");
    for (hash, expires_in, consumed_at) in [
        (&live, Duration::minutes(10), None),
        (&consumed, Duration::minutes(10), Some(Utc::now())),
        (&expired, Duration::minutes(-1), None),
    ] {
        sqlx::query(
            "INSERT INTO bridge_exchange_codes (code_hash, user_id, expires_at, consumed_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(hash)
        .bind(&user_id)
        .bind(Utc::now() + expires_in)
        .bind(consumed_at)
        .execute(&pg)
        .await
        .expect("insert exchange code fixture");
    }

    let removed = repo
        .delete_spent_bridge_exchange_codes()
        .await
        .expect("sweep");
    assert!(removed >= 2);

    let remaining: Vec<String> = sqlx::query_scalar(
        "SELECT code_hash FROM bridge_exchange_codes WHERE user_id = $1 ORDER BY code_hash",
    )
    .bind(&user_id)
    .fetch_all(&pg)
    .await
    .expect("remaining codes");
    assert_eq!(remaining, vec![live.clone()], "only the live, unconsumed code survives");

    let _ = sqlx::query("DELETE FROM bridge_exchange_codes WHERE user_id = $1")
        .bind(&user_id)
        .execute(&pg)
        .await;
    remove_user_and_client(&pg, &user_id, &client_id).await;
}
