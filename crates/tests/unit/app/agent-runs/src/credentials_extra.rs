use std::time::Duration;

use systemprompt_agent_runs::AgentRunsError;
use systemprompt_agent_runs::credentials::{
    IssueRunCredentials, RunCredentialLimits, RunCredentialService,
};
use systemprompt_agent_runs::repository::RunCredentialRepositories;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::DisposableDb;

const ISSUER: &str = "https://issuer.headless-extra.test";

async fn seed_user(pool: &systemprompt_database::DbPool) -> UserId {
    let id = UserId::new(uuid::Uuid::new_v4().to_string());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
        .bind(id.as_str())
        .bind(format!("runx-{}", id.as_str()))
        .bind(format!("{}@headless-extra.test", id.as_str()))
        .execute(pool.write_pool().as_ref())
        .await
        .expect("seed user");
    id
}

fn request(user_id: &UserId, ttl: Duration, purpose: Option<&str>) -> IssueRunCredentials {
    IssueRunCredentials {
        user_id: user_id.clone(),
        name: "headless:extra-run".to_owned(),
        ttl,
        scopes: Vec::new(),
        plugin_id: None,
        limits: RunCredentialLimits {
            max_budget_usd: None,
            max_requests: None,
        },
        purpose: purpose.map(str::to_owned),
    }
}

async fn live_keys(pool: &systemprompt_database::DbPool, user: &UserId) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_api_keys WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user.as_str())
    .fetch_one(pool.write_pool().as_ref())
    .await
    .expect("count keys")
}

#[tokio::test]
async fn a_ttl_beyond_chrono_range_is_refused_before_anything_is_issued() {
    let database = DisposableDb::with_schema("agent_run_credentials_ttl").await;
    let pool = database.test_pool().await;
    let user = seed_user(&pool).await;
    let service = RunCredentialService::new(
        RunCredentialRepositories::new(
            &pool,
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
        ),
        ISSUER,
        systemprompt_users::ApiKeyLifetime::unbounded(),
        systemprompt_test_fixtures::test_signing_keys(),
        systemprompt_test_fixtures::test_config(),
    );

    let err = service
        .issue(request(&user, Duration::MAX, None))
        .await
        .expect_err("an unrepresentable ttl is refused");
    assert!(
        matches!(err, AgentRunsError::CredentialTtl { .. }),
        "got {err:?}"
    );
    assert_eq!(live_keys(&pool, &user).await, 0);
    pool.write_pool().close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn issue_without_purpose_mints_no_mcp_token() {
    let database = DisposableDb::with_schema("agent_run_credentials_nompc").await;
    let pool = database.test_pool().await;
    let user = seed_user(&pool).await;
    let service = RunCredentialService::new(
        RunCredentialRepositories::new(
            &pool,
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
        ),
        ISSUER,
        systemprompt_users::ApiKeyLifetime::unbounded(),
        systemprompt_test_fixtures::test_signing_keys(),
        systemprompt_test_fixtures::test_config(),
    );

    let creds = service
        .issue(request(&user, Duration::from_secs(600), None))
        .await
        .expect("issue");
    assert!(creds.mcp_token.is_none());
    assert!(creds.mcp_token_jti.is_none());
    assert!(creds.mcp_token_expires_at.is_none());
    assert_eq!(live_keys(&pool, &user).await, 1);

    service.revoke(&creds).await.expect("revoke");
    assert_eq!(live_keys(&pool, &user).await, 0);
    pool.write_pool().close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn revoke_revokes_the_mcp_token_jti() {
    let database = DisposableDb::with_schema("agent_run_credentials_mcpjti").await;
    let pool = database.test_pool().await;
    let user = seed_user(&pool).await;
    let service = RunCredentialService::new(
        RunCredentialRepositories::new(
            &pool,
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
        ),
        ISSUER,
        systemprompt_users::ApiKeyLifetime::unbounded(),
        systemprompt_test_fixtures::test_signing_keys(),
        systemprompt_test_fixtures::test_config(),
    );

    let creds = service
        .issue(request(&user, Duration::from_secs(600), Some("farm:mcp")))
        .await
        .expect("issue");
    let jti = creds.mcp_token_jti.clone().expect("mcp jti");
    assert!(creds.mcp_token.is_some());
    assert!(creds.mcp_token_expires_at.is_some());
    assert!(
        format!("{service:?}").contains(ISSUER),
        "debug names the issuer"
    );

    service.revoke(&creds).await.expect("revoke");
    let revoked = systemprompt_test_fixtures::oauth_fixture_repository(&pool)
        .is_jti_revoked(&jti)
        .await
        .expect("jti lookup");
    assert!(revoked, "mcp token jti revoked");
    pool.write_pool().close().await;
    database.drop_now().await;
}
