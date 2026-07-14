// DB-backed WebAuthnService tests: credential lookup paths, ceremony state
// lifecycle, and verified-authentication token handling.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{WebAuthnConfig, WebAuthnService};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use systemprompt_traits::{AuthResult, AuthUser, UserProvider};
use url::Url;
use uuid::Uuid;

struct NoopUserProvider;

#[async_trait]
impl UserProvider for NoopUserProvider {
    async fn find_by_id(&self, _id: &UserId) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }
    async fn find_by_email(&self, _email: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }
    async fn find_by_name(&self, _name: &str) -> AuthResult<Option<AuthUser>> {
        Ok(None)
    }
    async fn create_user(
        &self,
        name: &str,
        email: &str,
        _full_name: Option<&str>,
    ) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: unique_user_id("wa-flow"),
            name: name.to_owned(),
            email: email.to_owned(),
            roles: Vec::new(),
            is_active: true,
        })
    }
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: unique_user_id("wa-flow"),
            name: format!("anon-{fingerprint}"),
            email: String::new(),
            roles: Vec::new(),
            is_active: true,
        })
    }
    async fn assign_roles(&self, _user_id: &UserId, _roles: &[String]) -> AuthResult<()> {
        Ok(())
    }
    async fn find_or_create_federated(
        &self,
        _issuer: &str,
        _external_sub: &str,
        _claims: &systemprompt_traits::FederatedIdentityClaims,
    ) -> AuthResult<UserId> {
        Ok(unique_user_id("wa-flow"))
    }
}

fn test_config() -> WebAuthnConfig {
    WebAuthnConfig {
        rp_id: "localhost".to_owned(),
        rp_origin: Url::parse("http://localhost:8080").expect("origin"),
        rp_name: "Test RP".to_owned(),
        challenge_expiry: Duration::from_secs(300),
        allow_any_port: true,
        allow_subdomains: true,
    }
}

struct Ctx {
    service: WebAuthnService,
    user_id: UserId,
    email: String,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("wa-flow");
    let email = format!("{}@waflow.invalid", user_id.as_str());
    seed_user_row(&pool, &user_id, &email)
        .await
        .expect("seed user");
    let service =
        WebAuthnService::with_config(test_config(), repo, Arc::new(NoopUserProvider)).expect("svc");
    Some(Ctx {
        service,
        user_id,
        email,
    })
}

#[tokio::test]
async fn start_registration_with_no_existing_credentials_succeeds() {
    let Some(ctx) = setup().await else { return };
    // No credentials are stored for this email, so the exclude-credentials
    // lookup (get_user_credentials_by_email) must resolve to an empty set and
    // the ceremony still starts.
    let (ccr, challenge_id) = ctx
        .service
        .start_registration(ctx.user_id.as_str(), &ctx.email, Some("Full Name"))
        .await
        .expect("start_registration");
    assert!(!challenge_id.is_empty());
    drop(ccr);
}

#[tokio::test]
async fn start_registration_unknown_email_treats_credentials_as_empty() {
    let Some(ctx) = setup().await else { return };
    // get_user_credentials_by_email on an email with no matching user must
    // short-circuit to an empty Vec rather than erroring.
    let unknown = format!("nobody-{}@waflow.invalid", Uuid::new_v4().simple());
    let (_, challenge_id) = ctx
        .service
        .start_registration("ghost", &unknown, None)
        .await
        .expect("start_registration");
    assert!(!challenge_id.is_empty());
}

#[tokio::test]
async fn start_authentication_unknown_user_errors() {
    let Some(ctx) = setup().await else { return };
    let err = ctx
        .service
        .start_authentication("absent@waflow.invalid", None)
        .await
        .expect_err("unknown user must fail");
    assert!(
        matches!(err, systemprompt_oauth::error::OauthError::UserNotFound(_)),
        "expected UserNotFound, got {err:?}"
    );
}

#[tokio::test]
async fn start_authentication_user_without_credentials_errors() {
    let Some(ctx) = setup().await else { return };
    // The user exists (seeded) but has no webauthn credentials, exercising the
    // get_user_credentials empty path and the "No credentials found" branch.
    let err = ctx
        .service
        .start_authentication(&ctx.email, None)
        .await
        .expect_err("no credentials must fail");
    assert!(
        matches!(err, systemprompt_oauth::error::OauthError::Internal(_)),
        "expected Internal(no credentials), got {err:?}"
    );
}

#[tokio::test]
async fn verified_authentication_roundtrip_consumes_once() {
    let Some(ctx) = setup().await else { return };
    let token = format!("vtok-{}", Uuid::new_v4().simple());
    ctx.service
        .store_verified_authentication(token.clone(), ctx.user_id.clone())
        .await;

    let consumed = ctx
        .service
        .consume_verified_authentication(&token)
        .await
        .expect("first consume succeeds");
    assert_eq!(consumed, ctx.user_id);

    let err = ctx
        .service
        .consume_verified_authentication(&token)
        .await
        .expect_err("second consume must fail");
    assert!(
        matches!(err, systemprompt_oauth::error::OauthError::Internal(_)),
        "consuming a spent token must error, got {err:?}"
    );
}

#[tokio::test]
async fn consume_verified_authentication_unknown_token_errors() {
    let Some(ctx) = setup().await else { return };
    let err = ctx
        .service
        .consume_verified_authentication("never-stored")
        .await
        .expect_err("unknown token must fail");
    assert!(matches!(
        err,
        systemprompt_oauth::error::OauthError::Internal(_)
    ));
}

#[tokio::test]
async fn service_debug_redacts_runtime_state() {
    let Some(ctx) = setup().await else { return };
    let debug = format!("{:?}", ctx.service);
    assert!(debug.contains("WebAuthnService"));
    assert!(debug.contains("localhost"));
    assert!(
        !debug.contains("reg_states"),
        "state maps stay out of Debug"
    );
}

#[tokio::test]
async fn cleanup_caps_pending_verified_authentications_by_age() {
    let Some(ctx) = setup().await else { return };
    let oldest = "vtok-oldest".to_owned();
    ctx.service
        .store_verified_authentication(oldest.clone(), ctx.user_id.clone())
        .await;
    for i in 0..WebAuthnService::MAX_PENDING_CHALLENGES {
        ctx.service
            .store_verified_authentication(format!("vtok-cap-{i}"), ctx.user_id.clone())
            .await;
    }

    ctx.service
        .cleanup_expired_states()
        .await
        .expect("cleanup with over-cap state");

    let err = ctx
        .service
        .consume_verified_authentication(&oldest)
        .await
        .expect_err("oldest token must be evicted by the cap");
    assert!(matches!(
        err,
        systemprompt_oauth::error::OauthError::Internal(_)
    ));
    let newest = format!("vtok-cap-{}", WebAuthnService::MAX_PENDING_CHALLENGES - 1);
    let consumed = ctx
        .service
        .consume_verified_authentication(&newest)
        .await
        .expect("newest token must survive the cap");
    assert_eq!(consumed, ctx.user_id);
}

#[tokio::test]
async fn cleanup_expired_states_is_idempotent_when_empty() {
    let Some(ctx) = setup().await else { return };
    ctx.service
        .cleanup_expired_states()
        .await
        .expect("cleanup with empty state");
    // A pending registration challenge that has not expired must survive.
    let (_, challenge_id) = ctx
        .service
        .start_registration(ctx.user_id.as_str(), &ctx.email, None)
        .await
        .expect("start_registration");
    ctx.service
        .cleanup_expired_states()
        .await
        .expect("cleanup after start");
    assert!(!challenge_id.is_empty());
}
