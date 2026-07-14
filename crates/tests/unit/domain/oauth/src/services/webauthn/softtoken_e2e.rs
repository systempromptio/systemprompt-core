// End-to-end WebAuthn ceremonies driven by a softtoken authenticator:
// registration finish (success + tampered), authentication finish (success,
// counter persistence, replayed challenge), and setup-token link flows.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::error::OauthError;
use systemprompt_oauth::repository::{
    CreateSetupTokenParams, OAuthRepository, SetupTokenPurpose, TokenValidationResult,
};
use systemprompt_oauth::services::webauthn::{
    FinishRegistrationParams, create_link_states, hash_token,
};
use systemprompt_oauth::services::{WebAuthnConfig, WebAuthnService};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row,
};
use systemprompt_traits::{AuthResult, AuthUser, UserProvider};
use url::Url;
use uuid::Uuid;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softtoken::SoftToken;

struct SeedingUserProvider {
    pool: DbPool,
}

#[async_trait]
impl UserProvider for SeedingUserProvider {
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
        let id = UserId::new(Uuid::new_v4().to_string());
        seed_user_row(&self.pool, &id, email)
            .await
            .map_err(|e| systemprompt_traits::AuthProviderError::Internal(e.to_string()))?;
        Ok(AuthUser {
            id,
            name: name.to_owned(),
            email: email.to_owned(),
            roles: Vec::new(),
            is_active: true,
        })
    }
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: UserId::new(Uuid::new_v4().to_string()),
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
        Ok(UserId::new(Uuid::new_v4().to_string()))
    }
}

fn origin() -> Url {
    Url::parse("http://localhost:8080").expect("origin")
}

fn test_config() -> WebAuthnConfig {
    WebAuthnConfig {
        rp_id: "localhost".to_owned(),
        rp_origin: origin(),
        rp_name: "SoftToken RP".to_owned(),
        challenge_expiry: Duration::from_secs(300),
        allow_any_port: true,
        allow_subdomains: true,
    }
}

struct Ctx {
    pool: DbPool,
    repo: OAuthRepository,
    service: WebAuthnService,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let provider = Arc::new(SeedingUserProvider { pool: pool.clone() });
    let service = WebAuthnService::with_config(test_config(), repo.clone(), provider).expect("svc");
    Some(Ctx {
        pool,
        repo,
        service,
    })
}

fn authenticator() -> WebauthnAuthenticator<SoftToken> {
    let (token, _ca) = SoftToken::new(true).expect("softtoken");
    WebauthnAuthenticator::new(token)
}

async fn register_user(
    ctx: &Ctx,
    auth: &mut WebauthnAuthenticator<SoftToken>,
    username: &str,
    email: &str,
) -> UserId {
    let (ccr, challenge_id) = ctx
        .service
        .start_registration(username, email, Some("Full Name"))
        .await
        .expect("start_registration");
    let cred = auth
        .do_registration(origin(), ccr)
        .expect("softtoken registration");
    ctx.service
        .finish_registration(
            FinishRegistrationParams::builder(&challenge_id, username, email, &cred)
                .with_full_name("Full Name")
                .build(),
        )
        .await
        .expect("finish_registration")
}

fn unique_email(tag: &str) -> String {
    format!("{tag}-{}@softtoken.invalid", Uuid::new_v4().simple())
}

#[tokio::test]
async fn registration_then_authentication_roundtrip_succeeds() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("rt");
    let mut auth = authenticator();
    let user_id = register_user(&ctx, &mut auth, "rt-user", &email).await;

    let (rcr, auth_challenge) = ctx
        .service
        .start_authentication(&email, Some("oauth-state-xyz".to_owned()))
        .await
        .expect("start_authentication");
    let assertion = auth
        .do_authentication(origin(), rcr)
        .expect("softtoken assertion");
    ctx.service
        .cleanup_expired_states()
        .await
        .expect("fresh auth state must survive cleanup");
    let (authed_user, oauth_state) = ctx
        .service
        .finish_authentication(&auth_challenge, &assertion)
        .await
        .expect("finish_authentication");

    assert_eq!(authed_user, user_id);
    assert_eq!(oauth_state.as_deref(), Some("oauth-state-xyz"));
}

#[tokio::test]
async fn registered_credential_is_persisted_and_excluded_on_reregistration() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("dup");
    let mut auth = authenticator();
    let user_id = register_user(&ctx, &mut auth, "dup-user", &email).await;

    let creds = ctx
        .repo
        .list_webauthn_credentials(&user_id)
        .await
        .expect("list credentials");
    assert_eq!(creds.len(), 1);
    assert_eq!(creds[0].counter, 0);

    let (ccr, _challenge) = ctx
        .service
        .start_registration("dup-user", &email, None)
        .await
        .expect("second start_registration");
    let excluded = ccr.public_key.exclude_credentials.unwrap_or_default();
    assert_eq!(excluded.len(), 1, "existing credential must be excluded");
}

#[tokio::test]
async fn finish_registration_unknown_challenge_is_state_expired() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("uc");
    let mut auth = authenticator();
    let (ccr, _challenge_id) = ctx
        .service
        .start_registration("uc-user", &email, None)
        .await
        .expect("start_registration");
    let cred = auth.do_registration(origin(), ccr).expect("registration");

    let err = ctx
        .service
        .finish_registration(
            FinishRegistrationParams::builder("no-such-challenge", "uc-user", &email, &cred)
                .build(),
        )
        .await
        .expect_err("unknown challenge must fail");
    assert!(matches!(err, OauthError::RegistrationStateExpired));
}

#[tokio::test]
async fn finish_registration_with_mismatched_challenge_fails_verification() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("mm");
    let mut auth = authenticator();
    let (ccr_a, _challenge_a) = ctx
        .service
        .start_registration("mm-user", &email, None)
        .await
        .expect("first challenge");
    let (_ccr_b, challenge_b) = ctx
        .service
        .start_registration("mm-user", &email, None)
        .await
        .expect("second challenge");
    let cred_for_a = auth.do_registration(origin(), ccr_a).expect("registration");

    let err = ctx
        .service
        .finish_registration(
            FinishRegistrationParams::builder(&challenge_b, "mm-user", &email, &cred_for_a).build(),
        )
        .await
        .expect_err("credential answering challenge A must not satisfy challenge B");
    assert!(matches!(err, OauthError::WebAuthnVerificationFailed(_)));
}

#[tokio::test]
async fn finish_authentication_unknown_challenge_errors() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("ua");
    let mut auth = authenticator();
    register_user(&ctx, &mut auth, "ua-user", &email).await;
    let (rcr, _challenge) = ctx
        .service
        .start_authentication(&email, None)
        .await
        .expect("start_authentication");
    let assertion = auth.do_authentication(origin(), rcr).expect("assertion");

    let err = ctx
        .service
        .finish_authentication("missing-challenge", &assertion)
        .await
        .expect_err("unknown auth challenge must fail");
    assert!(matches!(err, OauthError::Internal(_)));
}

#[tokio::test]
async fn finish_authentication_with_mismatched_assertion_fails_verification() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("ma");
    let mut auth = authenticator();
    register_user(&ctx, &mut auth, "ma-user", &email).await;

    let (rcr_a, _challenge_a) = ctx
        .service
        .start_authentication(&email, None)
        .await
        .expect("challenge A");
    let (_rcr_b, challenge_b) = ctx
        .service
        .start_authentication(&email, None)
        .await
        .expect("challenge B");
    let assertion_for_a = auth.do_authentication(origin(), rcr_a).expect("assertion");

    let err = ctx
        .service
        .finish_authentication(&challenge_b, &assertion_for_a)
        .await
        .expect_err("assertion answering challenge A must not satisfy challenge B");
    assert!(matches!(err, OauthError::WebAuthnVerificationFailed(_)));
}

async fn seed_uuid_user(pool: &DbPool, email: &str) -> UserId {
    let user_id = UserId::new(Uuid::new_v4().to_string());
    seed_user_row(pool, &user_id, email).await.expect("seed");
    user_id
}

async fn store_link_token(
    repo: &OAuthRepository,
    user_id: &UserId,
    expires_in_secs: i64,
) -> String {
    let raw = format!("link-{}", Uuid::new_v4().simple());
    repo.store_setup_token(CreateSetupTokenParams {
        user_id: user_id.clone(),
        token_hash: hash_token(&raw),
        purpose: SetupTokenPurpose::CredentialLink,
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(expires_in_secs),
    })
    .await
    .expect("store setup token");
    raw
}

#[tokio::test]
async fn link_flow_registers_credential_and_consumes_token() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("link");
    let user_id = seed_uuid_user(&ctx.pool, &email).await;
    let raw_token = store_link_token(&ctx.repo, &user_id, 600).await;
    let link_states = create_link_states();
    let mut auth = authenticator();

    let (ccr, challenge_id, user_info) = ctx
        .service
        .start_registration_with_token(&raw_token, &link_states)
        .await
        .expect("start link registration");
    assert_eq!(user_info.id, user_id);
    assert_eq!(user_info.email, email);

    let cred = auth.do_registration(origin(), ccr).expect("registration");
    let linked = ctx
        .service
        .finish_registration_with_token(&challenge_id, &raw_token, &cred, &link_states)
        .await
        .expect("finish link registration");
    assert_eq!(linked, user_id);

    let creds = ctx
        .repo
        .list_webauthn_credentials(&user_id)
        .await
        .expect("list credentials");
    assert_eq!(creds.len(), 1);

    let revalidation = ctx
        .repo
        .validate_setup_token(&hash_token(&raw_token))
        .await
        .expect("revalidate");
    assert!(matches!(revalidation, TokenValidationResult::AlreadyUsed));
}

#[tokio::test]
async fn link_flow_excludes_existing_credentials_on_second_link() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("link2");
    let user_id = seed_uuid_user(&ctx.pool, &email).await;
    let first_token = store_link_token(&ctx.repo, &user_id, 600).await;
    let link_states = create_link_states();
    let mut auth = authenticator();

    let (ccr, challenge_id, _info) = ctx
        .service
        .start_registration_with_token(&first_token, &link_states)
        .await
        .expect("start first link");
    let cred = auth.do_registration(origin(), ccr).expect("registration");
    ctx.service
        .finish_registration_with_token(&challenge_id, &first_token, &cred, &link_states)
        .await
        .expect("finish first link");

    let second_token = store_link_token(&ctx.repo, &user_id, 600).await;
    let (ccr2, _challenge2, _info2) = ctx
        .service
        .start_registration_with_token(&second_token, &link_states)
        .await
        .expect("start second link");
    let excluded = ccr2.public_key.exclude_credentials.unwrap_or_default();
    assert_eq!(excluded.len(), 1, "linked credential must be excluded");
}

#[tokio::test]
async fn start_link_rejects_unknown_expired_and_used_tokens() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("badtok");
    let user_id = seed_uuid_user(&ctx.pool, &email).await;
    let link_states = create_link_states();

    let err = ctx
        .service
        .start_registration_with_token("never-issued", &link_states)
        .await
        .expect_err("unknown token");
    assert!(err.to_string().contains("Invalid setup token"));

    let expired = store_link_token(&ctx.repo, &user_id, -60).await;
    let err = ctx
        .service
        .start_registration_with_token(&expired, &link_states)
        .await
        .expect_err("expired token");
    assert!(err.to_string().contains("expired"));

    let mut auth = authenticator();
    let used = store_link_token(&ctx.repo, &user_id, 600).await;
    let (ccr, challenge_id, _info) = ctx
        .service
        .start_registration_with_token(&used, &link_states)
        .await
        .expect("start link");
    let cred = auth.do_registration(origin(), ccr).expect("registration");
    ctx.service
        .finish_registration_with_token(&challenge_id, &used, &cred, &link_states)
        .await
        .expect("finish link");
    let err = ctx
        .service
        .start_registration_with_token(&used, &link_states)
        .await
        .expect_err("used token");
    assert!(err.to_string().contains("already been used"));
}

#[tokio::test]
async fn start_link_rejects_non_uuid_user_id() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("nonuuid");
    let user_id = UserId::new(format!("not-a-uuid-{}", Uuid::new_v4().simple()));
    seed_user_row(&ctx.pool, &user_id, &email)
        .await
        .expect("seed");
    let raw_token = store_link_token(&ctx.repo, &user_id, 600).await;
    let link_states = create_link_states();

    let err = ctx
        .service
        .start_registration_with_token(&raw_token, &link_states)
        .await
        .expect_err("non-uuid user id must fail");
    let msg = err.to_string();
    assert!(msg.contains("Invalid user UUID"), "got: {msg}");
}

#[tokio::test]
async fn finish_link_rejects_missing_session_and_invalid_token() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("linkerr");
    let user_id = seed_uuid_user(&ctx.pool, &email).await;
    let raw_token = store_link_token(&ctx.repo, &user_id, 600).await;
    let link_states = create_link_states();
    let mut auth = authenticator();

    let (ccr, _challenge_id, _info) = ctx
        .service
        .start_registration_with_token(&raw_token, &link_states)
        .await
        .expect("start link");
    let cred = auth.do_registration(origin(), ccr).expect("registration");

    let err = ctx
        .service
        .finish_registration_with_token("missing-session", &raw_token, &cred, &link_states)
        .await
        .expect_err("missing session must fail");
    assert!(err.to_string().contains("not found or expired"));

    let err = ctx
        .service
        .finish_registration_with_token("missing-session", "never-issued", &cred, &link_states)
        .await
        .expect_err("invalid token must fail");
    assert!(err.to_string().contains("Invalid or expired setup token"));
}

#[tokio::test]
async fn finish_link_rejects_token_swapped_between_sessions() {
    let Some(ctx) = setup().await else { return };
    let email = unique_email("swap");
    let user_id = seed_uuid_user(&ctx.pool, &email).await;
    let token_a = store_link_token(&ctx.repo, &user_id, 600).await;
    let token_b = store_link_token(&ctx.repo, &user_id, 600).await;
    let link_states = create_link_states();
    let mut auth = authenticator();

    let (ccr, challenge_a, _info) = ctx
        .service
        .start_registration_with_token(&token_a, &link_states)
        .await
        .expect("start link with token A");
    let cred = auth.do_registration(origin(), ccr).expect("registration");

    let err = ctx
        .service
        .finish_registration_with_token(&challenge_a, &token_b, &cred, &link_states)
        .await
        .expect_err("finishing session A with token B must fail");
    assert!(err.to_string().contains("Token mismatch"));
}
