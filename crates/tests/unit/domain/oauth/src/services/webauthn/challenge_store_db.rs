// WebAuthn challenge state lives in Postgres: a ceremony started on one
// service instance (replica A) must finish on another (replica B), consume
// exactly once, and honour the stored TTL.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{TokenId, UserId};
use systemprompt_oauth::error::OauthError;
use systemprompt_oauth::repository::{
    CreateSetupTokenParams, LinkChallengeReservation, OAuthRepository, ReserveLinkChallengeParams,
    SetupTokenPurpose, StoreChallengeParams, WebAuthnChallengeKind,
};
use systemprompt_oauth::services::webauthn::hash_token;
use systemprompt_oauth::services::{WebAuthnConfig, WebAuthnService};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use systemprompt_traits::{AuthResult, AuthUser, UserProvider};
use url::Url;
use uuid::Uuid;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::softtoken::SoftToken;

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
            id: unique_user_id("wa-store"),
            name: name.to_owned(),
            email: email.to_owned(),
            roles: Vec::new(),
            is_active: true,
        })
    }
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser> {
        Ok(AuthUser {
            id: unique_user_id("wa-store"),
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
        Ok(unique_user_id("wa-store"))
    }
    async fn promote_anonymous(&self, _source: &UserId, _target: &UserId) -> AuthResult<u64> {
        Ok(0)
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
    pool: DbPool,
    repo: OAuthRepository,
    replica_a: WebAuthnService,
    replica_b: WebAuthnService,
    user_id: UserId,
    email: String,
}

async fn setup_or_skip() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("wa-store");
    let email = format!("{}@wastore.invalid", user_id.as_str());
    seed_user_row(&pool, &user_id, &email)
        .await
        .expect("seed user");
    let build = || {
        WebAuthnService::with_config(
            test_config(),
            OAuthRepository::new(&pool).expect("repo"),
            Arc::new(NoopUserProvider),
        )
        .expect("svc")
    };
    Some(Ctx {
        pool: pool.clone(),
        repo,
        replica_a: build(),
        replica_b: build(),
        user_id,
        email,
    })
}

#[tokio::test]
async fn registration_challenge_started_on_replica_a_is_consumable_on_replica_b() {
    let Some(ctx) = setup_or_skip().await else { return };
    let (_, challenge_id) = ctx
        .replica_a
        .start_registration("cross-replica", &ctx.email, None)
        .await
        .expect("start_registration");

    let consumed = ctx
        .repo
        .consume_webauthn_challenge(&challenge_id, WebAuthnChallengeKind::Registration)
        .await
        .expect("consume")
        .expect("challenge stored by replica A is visible through the shared pool");
    assert!(consumed.user_id.is_none());
    assert!(consumed.state.is_object());
    drop(ctx.replica_b);
}

#[tokio::test]
async fn challenge_consumes_exactly_once() {
    let Some(ctx) = setup_or_skip().await else { return };
    let challenge = format!("once-{}", Uuid::new_v4().simple());
    let state = serde_json::json!({"k": "v"});
    ctx.repo
        .store_webauthn_challenge(StoreChallengeParams {
            challenge: &challenge,
            kind: WebAuthnChallengeKind::Authentication,
            user_id: Some(&ctx.user_id),
            state: &state,
            oauth_state: Some("st"),
            ttl: Duration::from_secs(60),
        })
        .await
        .expect("store");

    let first = ctx
        .repo
        .consume_webauthn_challenge(&challenge, WebAuthnChallengeKind::Authentication)
        .await
        .expect("first consume")
        .expect("present");
    assert_eq!(first.user_id, Some(ctx.user_id.clone()));
    assert_eq!(first.state, state);
    assert_eq!(first.oauth_state.as_deref(), Some("st"));

    let second = ctx
        .repo
        .consume_webauthn_challenge(&challenge, WebAuthnChallengeKind::Authentication)
        .await
        .expect("second consume");
    assert!(
        second.is_none(),
        "a consumed challenge must not be replayable"
    );
}

#[tokio::test]
async fn challenge_kind_mismatch_does_not_consume() {
    let Some(ctx) = setup_or_skip().await else { return };
    let challenge = format!("kind-{}", Uuid::new_v4().simple());
    ctx.repo
        .store_webauthn_challenge(StoreChallengeParams {
            challenge: &challenge,
            kind: WebAuthnChallengeKind::Registration,
            user_id: None,
            state: &serde_json::Value::Null,
            oauth_state: None,
            ttl: Duration::from_secs(60),
        })
        .await
        .expect("store");

    let wrong_kind = ctx
        .repo
        .consume_webauthn_challenge(&challenge, WebAuthnChallengeKind::Verified)
        .await
        .expect("consume");
    assert!(wrong_kind.is_none());
    let right_kind = ctx
        .repo
        .consume_webauthn_challenge(&challenge, WebAuthnChallengeKind::Registration)
        .await
        .expect("consume");
    assert!(right_kind.is_some());
}

#[tokio::test]
async fn expired_challenge_is_not_consumable_and_is_purged_by_cleanup() {
    let Some(ctx) = setup_or_skip().await else { return };
    let challenge = format!("ttl0-{}", Uuid::new_v4().simple());
    ctx.repo
        .store_webauthn_challenge(StoreChallengeParams {
            challenge: &challenge,
            kind: WebAuthnChallengeKind::Registration,
            user_id: None,
            state: &serde_json::Value::Null,
            oauth_state: None,
            ttl: Duration::ZERO,
        })
        .await
        .expect("store");

    let consumed = ctx
        .repo
        .consume_webauthn_challenge(&challenge, WebAuthnChallengeKind::Registration)
        .await
        .expect("consume");
    assert!(consumed.is_none(), "ttl 0 must already be expired");

    ctx.replica_a
        .cleanup_expired_states()
        .await
        .expect("cleanup");
    let removed = ctx
        .repo
        .cleanup_expired_webauthn_challenges()
        .await
        .expect("second cleanup");
    assert_eq!(removed, 0, "the first cleanup purged the expired row");
}

#[tokio::test]
async fn verified_token_stored_on_replica_a_is_consumed_on_replica_b() {
    let Some(ctx) = setup_or_skip().await else { return };
    let token = format!("vtok-{}", Uuid::new_v4().simple());
    ctx.replica_a
        .store_verified_authentication(token.clone(), ctx.user_id.clone())
        .await
        .expect("store verified");

    let user = ctx
        .replica_b
        .consume_verified_authentication(&token)
        .await
        .expect("replica B consumes the token stored by replica A");
    assert_eq!(user, ctx.user_id);

    let err = ctx
        .replica_a
        .consume_verified_authentication(&token)
        .await
        .expect_err("token is single-use across replicas");
    assert!(matches!(err, OauthError::Internal(_)), "got {err:?}");
}

#[tokio::test]
async fn registration_state_expired_when_challenge_unknown() {
    let Some(ctx) = setup_or_skip().await else { return };
    let missing = ctx
        .repo
        .consume_webauthn_challenge("never-stored", WebAuthnChallengeKind::Registration)
        .await
        .expect("consume");
    assert!(missing.is_none());
    drop(ctx);
}

async fn store_link_token(repo: &OAuthRepository, user_id: &UserId) -> String {
    let raw = format!("link-{}", Uuid::new_v4().simple());
    repo.store_setup_token(CreateSetupTokenParams {
        user_id: user_id.clone(),
        token_hash: hash_token(&raw),
        purpose: SetupTokenPurpose::CredentialLink,
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(600),
    })
    .await
    .expect("store setup token");
    raw
}

#[tokio::test]
async fn link_started_on_replica_a_finishes_on_replica_b() {
    let Some(ctx) = setup_or_skip().await else { return };
    // The link ceremony needs a UUID user id: it becomes the passkey's
    // user handle.
    let pool = fixture_db_pool(&fixture_database_url().expect("url"))
        .await
        .expect("pool");
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{}@wastore.invalid", user_id.as_str());
    seed_user_row(&pool, &user_id, &email)
        .await
        .expect("seed uuid user");
    let raw_token = store_link_token(&ctx.repo, &user_id).await;

    let (ccr, challenge_id, info) = ctx
        .replica_a
        .start_registration_with_token(&raw_token)
        .await
        .expect("start link on replica A");
    assert_eq!(info.id, user_id);

    let (token, _ca) = SoftToken::new(true).expect("softtoken");
    let mut authenticator = WebauthnAuthenticator::new(token);
    let origin = Url::parse("http://localhost:8080").expect("origin");
    let cred = authenticator
        .do_registration(origin, ccr)
        .expect("softtoken registration");

    let linked = ctx
        .replica_b
        .finish_registration_with_token(&challenge_id, &raw_token, &cred)
        .await
        .expect("finish link on replica B");
    assert_eq!(linked, user_id);

    // The setup token is consumed by the successful finish, so the replay is
    // rejected at token validation before the challenge row is even looked up.
    let replay = ctx
        .replica_a
        .finish_registration_with_token(&challenge_id, &raw_token, &cred)
        .await
        .expect_err("a finished link ceremony cannot be replayed");
    assert!(matches!(replay, OauthError::Internal(_)), "{replay}");
    let row = ctx
        .repo
        .consume_webauthn_challenge(&challenge_id, WebAuthnChallengeKind::Link)
        .await
        .expect("consume");
    assert!(
        row.is_none(),
        "the link challenge row was consumed by replica B"
    );
}

async fn link_rows(pool: &DbPool, user_id: &UserId) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM webauthn_challenges WHERE user_id = $1 AND challenge_type = 'link'",
    )
    .bind(user_id.as_str())
    .fetch_one(&*pool.pool().expect("pool"))
    .await
    .expect("count link rows")
}

fn link_state(token_id: &TokenId, marker: u32) -> serde_json::Value {
    serde_json::json!({ "token_id": token_id.as_str(), "marker": marker })
}

async fn reserve(
    repo: &OAuthRepository,
    user_id: &UserId,
    token_id: &TokenId,
    ttl: Duration,
    marker: u32,
) -> LinkChallengeReservation {
    repo.reserve_link_challenge(
        ReserveLinkChallengeParams {
            user_id,
            token_id,
            ttl,
            min_remaining: Duration::from_secs(60),
        },
        |_| Ok(link_state(token_id, marker)),
    )
    .await
    .expect("reserve link challenge")
}

#[tokio::test]
async fn reserve_link_challenge_returns_the_live_challenge_for_the_same_token() {
    let ctx = setup_or_skip().await.expect("DATABASE_URL must be set");
    let token = TokenId::generate();

    let first = reserve(&ctx.repo, &ctx.user_id, &token, Duration::from_secs(300), 1).await;
    let second = reserve(&ctx.repo, &ctx.user_id, &token, Duration::from_secs(300), 2).await;

    assert!(!first.reused);
    assert!(
        second.reused,
        "an overlapping start must reuse the live ceremony"
    );
    assert_eq!(second.challenge_id, first.challenge_id);
    assert_eq!(
        second.state,
        link_state(&token, 1),
        "the reused state is the first mint"
    );
    assert_eq!(link_rows(&ctx.pool, &ctx.user_id).await, 1);
}

#[tokio::test]
async fn reserve_link_challenge_replaces_a_challenge_issued_for_another_token() {
    let ctx = setup_or_skip().await.expect("DATABASE_URL must be set");
    let token_a = TokenId::generate();
    let token_b = TokenId::generate();

    let first = reserve(
        &ctx.repo,
        &ctx.user_id,
        &token_a,
        Duration::from_secs(300),
        1,
    )
    .await;
    let second = reserve(
        &ctx.repo,
        &ctx.user_id,
        &token_b,
        Duration::from_secs(300),
        2,
    )
    .await;

    assert!(!second.reused);
    assert_ne!(second.challenge_id, first.challenge_id);
    assert_eq!(link_rows(&ctx.pool, &ctx.user_id).await, 1);
    let stale = ctx
        .repo
        .consume_webauthn_challenge(&first.challenge_id, WebAuthnChallengeKind::Link)
        .await
        .expect("consume");
    assert!(stale.is_none(), "the superseded challenge must be gone");
}

#[tokio::test]
async fn reserve_link_challenge_replaces_a_near_expiry_challenge() {
    let ctx = setup_or_skip().await.expect("DATABASE_URL must be set");
    let token = TokenId::generate();

    let first = reserve(&ctx.repo, &ctx.user_id, &token, Duration::from_secs(30), 1).await;
    let second = reserve(&ctx.repo, &ctx.user_id, &token, Duration::from_secs(300), 2).await;

    assert!(
        !second.reused,
        "a challenge with less than a minute left must not be handed out again"
    );
    assert_ne!(second.challenge_id, first.challenge_id);
    assert_eq!(link_rows(&ctx.pool, &ctx.user_id).await, 1);
}

#[tokio::test]
async fn reserve_link_challenge_ignores_other_kinds_and_other_users() {
    let ctx = setup_or_skip().await.expect("DATABASE_URL must be set");
    let other = unique_user_id("wa-other");
    seed_user_row(
        &ctx.pool,
        &other,
        &format!("{}@wastore.invalid", other.as_str()),
    )
    .await
    .expect("seed other user");
    let auth_challenge = format!("auth-{}", Uuid::new_v4().simple());
    ctx.repo
        .store_webauthn_challenge(StoreChallengeParams {
            challenge: &auth_challenge,
            kind: WebAuthnChallengeKind::Authentication,
            user_id: Some(&ctx.user_id),
            state: &serde_json::Value::Null,
            oauth_state: None,
            ttl: Duration::from_secs(60),
        })
        .await
        .expect("store auth challenge");
    let theirs = reserve(
        &ctx.repo,
        &other,
        &TokenId::generate(),
        Duration::from_secs(300),
        1,
    )
    .await;

    reserve(
        &ctx.repo,
        &ctx.user_id,
        &TokenId::generate(),
        Duration::from_secs(300),
        2,
    )
    .await;
    reserve(
        &ctx.repo,
        &ctx.user_id,
        &TokenId::generate(),
        Duration::from_secs(300),
        3,
    )
    .await;

    assert!(
        ctx.repo
            .consume_webauthn_challenge(&auth_challenge, WebAuthnChallengeKind::Authentication)
            .await
            .expect("consume")
            .is_some(),
        "an authentication challenge for the same user must survive a link reservation"
    );
    assert!(
        ctx.repo
            .consume_webauthn_challenge(&theirs.challenge_id, WebAuthnChallengeKind::Link)
            .await
            .expect("consume")
            .is_some(),
        "another user's link challenge must survive"
    );
}

#[tokio::test]
async fn concurrent_reservations_converge_on_one_challenge() {
    let ctx = setup_or_skip().await.expect("DATABASE_URL must be set");
    let token = TokenId::generate();
    let mints = Arc::new(AtomicUsize::new(0));

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let repo = OAuthRepository::new(&ctx.pool).expect("repo");
        let user_id = ctx.user_id.clone();
        let token = token.clone();
        let mints = Arc::clone(&mints);
        tasks.spawn(async move {
            repo.reserve_link_challenge(
                ReserveLinkChallengeParams {
                    user_id: &user_id,
                    token_id: &token,
                    ttl: Duration::from_secs(300),
                    min_remaining: Duration::from_secs(60),
                },
                |_| {
                    mints.fetch_add(1, Ordering::SeqCst);
                    Ok(link_state(&token, 1))
                },
            )
            .await
            .expect("reserve")
            .challenge_id
        });
    }
    let mut ids = Vec::new();
    while let Some(id) = tasks.join_next().await {
        ids.push(id.expect("task"));
    }

    assert_eq!(ids.len(), 8);
    assert!(
        ids.iter().all(|id| id == &ids[0]),
        "every concurrent start must receive the same challenge id: {ids:?}"
    );
    assert_eq!(
        mints.load(Ordering::SeqCst),
        1,
        "exactly one ceremony is minted"
    );
    assert_eq!(link_rows(&ctx.pool, &ctx.user_id).await, 1);
}
