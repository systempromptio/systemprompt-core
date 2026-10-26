//! DB-backed tests for `RunCredentialService`: issue mints a verifiable PAT
//! with the run's scopes, a gateway session with the run's ttl and a hook
//! JWT the hook validator accepts; revoke undoes all three.

use std::time::Duration;

use systemprompt_agent_runs::credentials::{
    IssueRunCredentials, RunCredentialLimits, RunCredentialService,
};
use systemprompt_agent_runs::repository::RunCredentialRepositories;
use systemprompt_identifiers::{PluginId, ScopeDimension, UserId};
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_models::auth::Permission;
use systemprompt_security::HookTokenValidator;
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_users::{ApiKeyService, SessionRepository, UserRepository};

const ISSUER: &str = "https://issuer.headless.test";

async fn seed_user(pool: &systemprompt_database::DbPool) -> UserId {
    let id = UserId::new(uuid::Uuid::new_v4().to_string());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
        .bind(id.as_str())
        .bind(format!("run-{}", id.as_str()))
        .bind(format!("{}@headless.test", id.as_str()))
        .execute(pool.write_pool().as_ref())
        .await
        .expect("seed user");
    id
}

fn request(user_id: &UserId, plugin: Option<&str>) -> IssueRunCredentials {
    IssueRunCredentials {
        user_id: user_id.clone(),
        name: "headless:test-run".to_owned(),
        ttl: Duration::from_secs(1800),
        scopes: vec![ScopeBinding {
            dimension: ScopeDimension::new("project"),
            value: "farm-alpha".to_owned(),
        }],
        plugin_id: plugin.map(PluginId::new),
        limits: RunCredentialLimits {
            max_budget_usd: Some(2.5),
            max_requests: Some(40),
        },
        purpose: Some("farm:test-run".to_owned()),
    }
}

async fn session_expiry_secs(pool: &systemprompt_database::DbPool, id: &str) -> Option<i64> {
    sqlx::query_scalar::<_, Option<f64>>(
        "SELECT EXTRACT(EPOCH FROM (expires_at - now()))::float8 FROM user_sessions WHERE session_id = $1",
    )
    .bind(id)
    .fetch_optional(pool.write_pool().as_ref())
    .await
    .expect("query session")
    .flatten()
    .map(|secs| secs.round() as i64)
}

#[tokio::test]
async fn issue_then_revoke_round_trips_every_credential() {
    let database = DisposableDb::with_schema("agent_run_credentials").await;
    let pool = database.test_pool().await;
    let user = seed_user(&pool).await;
    let service = RunCredentialService::new(
        RunCredentialRepositories::new(
            &pool,
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
        ),
        systemprompt_users::ApiKeyLifetime::unbounded(),
        systemprompt_test_fixtures::test_token_authority(ISSUER),
        systemprompt_test_fixtures::test_config(),
    );

    let creds = service
        .issue(request(&user, Some("farm-governance")))
        .await
        .expect("issue run credentials");
    assert_eq!(creds.user_id, user);

    let keys = ApiKeyService::new(
        std::sync::Arc::new(UserRepository::new(&pool)),
        systemprompt_users::ApiKeyLifetime::unbounded(),
    );
    let key = keys
        .verify(creds.pat.as_str())
        .await
        .expect("verify")
        .expect("PAT verifies");
    assert_eq!(key.id, creds.api_key_id);
    assert_eq!(key.user_id, user);
    assert_eq!(key.scopes.len(), 1);
    assert_eq!(key.scopes[0].dimension.as_str(), "project");
    assert_eq!(key.scopes[0].value, "farm-alpha");
    assert_eq!(key.limits.budget_microdollars, Some(2_500_000));
    assert_eq!(key.limits.max_requests, Some(40));
    assert_eq!(key.limits.request_window_seconds, Some(1800));

    let left = session_expiry_secs(&pool, creds.gateway_session_id.as_str())
        .await
        .expect("session row with expiry");
    assert!(
        (1790..=1800).contains(&left),
        "session ttl ~1800s, got {left}"
    );
    let sessions = SessionRepository::new(&pool);
    assert!(
        sessions
            .find_active_by_id(&creds.gateway_session_id)
            .await
            .expect("lookup")
            .is_some()
    );

    let token = creds.hook_token.as_deref().expect("hook token");
    let claims = HookTokenValidator::new(std::sync::Arc::new(
        systemprompt_security::DeploymentTokenAuthority::new(
            systemprompt_test_fixtures::test_signing_keys(),
            ISSUER.to_owned(),
        ),
    ))
    .validate_govern(token, Some(&PluginId::new("farm-governance")))
    .expect("hook token validates for govern");
    assert_eq!(claims.subject, user);
    assert_eq!(claims.plugin_id.as_str(), "farm-governance");
    assert!(claims.scopes.contains(&Permission::HookTrack));
    assert!(creds.hook_token_jti.is_some());

    service.revoke(&creds).await.expect("revoke");
    assert!(
        keys.verify(creds.pat.as_str())
            .await
            .expect("verify")
            .is_none()
    );
    assert!(
        sessions
            .find_active_by_id(&creds.gateway_session_id)
            .await
            .expect("lookup")
            .is_none()
    );
    let revoked = systemprompt_test_fixtures::oauth_fixture_repository(&pool)
        .is_jti_revoked(creds.hook_token_jti.as_ref().expect("jti"))
        .await
        .expect("jti lookup");
    assert!(revoked, "hook token jti revoked");

    pool.write_pool().close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn issue_without_plugin_mints_no_hook_token() {
    let database = DisposableDb::with_schema("agent_run_credentials_noplugin").await;
    let pool = database.test_pool().await;
    let user = seed_user(&pool).await;
    let service = RunCredentialService::new(
        RunCredentialRepositories::new(
            &pool,
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
        ),
        systemprompt_users::ApiKeyLifetime::unbounded(),
        systemprompt_test_fixtures::test_token_authority(ISSUER),
        systemprompt_test_fixtures::test_config(),
    );
    let creds = service.issue(request(&user, None)).await.expect("issue");
    assert!(creds.hook_token.is_none());
    assert!(creds.hook_token_jti.is_none());
    service.revoke(&creds).await.expect("revoke");
    pool.write_pool().close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn issue_for_unknown_user_leaves_no_live_key() {
    let database = DisposableDb::with_schema("agent_run_credentials_nouser").await;
    let pool = database.test_pool().await;
    let ghost = UserId::new(uuid::Uuid::new_v4().to_string());
    let service = RunCredentialService::new(
        RunCredentialRepositories::new(
            &pool,
            systemprompt_test_fixtures::oauth_fixture_repository(&pool),
        ),
        systemprompt_users::ApiKeyLifetime::unbounded(),
        systemprompt_test_fixtures::test_token_authority(ISSUER),
        systemprompt_test_fixtures::test_config(),
    );
    assert!(service.issue(request(&ghost, None)).await.is_err());
    let live: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_api_keys WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(ghost.as_str())
    .fetch_one(pool.write_pool().as_ref())
    .await
    .expect("count keys");
    assert_eq!(live, 0, "a failed issue leaves no live PAT");
    pool.write_pool().close().await;
    database.drop_now().await;
}
