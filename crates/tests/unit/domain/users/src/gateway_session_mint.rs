//! The gateway session mint verifies the presented API key and mints, keeps
//! or binds a session for that key's owner only: an unverified key mints
//! nothing, another user's or an ownerless session is never kept, and the
//! key's bound session is reused until it is gone.

use std::sync::Arc;

use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, seed_user_row, test_db_pool};
use systemprompt_traits::{GatewaySessionMint, SessionAnalytics};
use systemprompt_users::{
    ApiKeyKind, ApiKeyLifetime, ApiKeyLimits, ApiKeyService, IssueApiKeyParams, SessionRepository,
    UserRepository, UsersGatewaySessionMint,
};
use uuid::Uuid;

struct Rig {
    mint: UsersGatewaySessionMint,
    secret: String,
    user_id: UserId,
}

async fn seeded_user(pool: &systemprompt_database::DbPool, prefix: &str) -> UserId {
    let user_id = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{prefix}-{}@mint.invalid", Uuid::new_v4().simple());
    seed_user_row(pool, &user_id, &email).await.expect("user");
    user_id
}

async fn rig(prefix: &str) -> Rig {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let user_id = seeded_user(&pool, prefix).await;
    let users = Arc::new(UserRepository::new(&pool));
    let minted = ApiKeyService::new(Arc::clone(&users), ApiKeyLifetime::unbounded())
        .issue(IssueApiKeyParams {
            user_id: &user_id,
            name: "gateway",
            expires_at: None,
            limits: &ApiKeyLimits::default(),
            scopes: &[],
            purpose: None,
            kind: ApiKeyKind::User,
        })
        .await
        .expect("issue");
    Rig {
        mint: UsersGatewaySessionMint::new(
            users,
            SessionRepository::new(&pool),
            ApiKeyLifetime::unbounded(),
            chrono::Duration::hours(1),
        ),
        secret: minted.secret,
        user_id,
    }
}

#[tokio::test]
async fn a_verified_key_mints_a_session_for_its_owner_only() {
    let rig = rig("mint").await;
    let minted = rig
        .mint
        .mint_for_api_key(&rig.secret, &SessionAnalytics::default())
        .await
        .expect("mint")
        .expect("a verified key mints");
    assert_eq!(minted.user_id, rig.user_id);
    assert!(!minted.presented_session_kept);
}

#[tokio::test]
async fn an_unverified_key_mints_and_binds_nothing() {
    let rig = rig("unverified").await;
    let forged = format!("{}x", rig.secret);
    assert!(
        rig.mint
            .mint_for_api_key(&forged, &SessionAnalytics::default())
            .await
            .expect("mint")
            .is_none()
    );
    assert!(
        rig.mint
            .bind_for_api_key(&forged, None, &SessionAnalytics::default())
            .await
            .expect("bind")
            .is_none()
    );
}

#[tokio::test]
async fn the_bound_session_is_reused_and_another_users_session_is_never_kept() {
    let other = rig("bind-other").await;
    let rig = rig("bind").await;
    let analytics = SessionAnalytics::default();
    let first = rig
        .mint
        .bind_for_api_key(&rig.secret, None, &analytics)
        .await
        .expect("bind")
        .expect("bound");
    let again = rig
        .mint
        .bind_for_api_key(&rig.secret, None, &analytics)
        .await
        .expect("bind")
        .expect("bound");
    assert_eq!(first.session_id, again.session_id);

    let foreign = other
        .mint
        .mint_for_api_key(&other.secret, &analytics)
        .await
        .expect("mint")
        .expect("minted");
    let presented = rig
        .mint
        .bind_for_api_key(&rig.secret, Some(&foreign.session_id), &analytics)
        .await
        .expect("bind")
        .expect("bound");
    assert!(!presented.presented_session_kept);
    assert_eq!(presented.session_id, first.session_id);
    assert_eq!(presented.user_id, rig.user_id);

    let kept = rig
        .mint
        .bind_for_api_key(&rig.secret, Some(&first.session_id), &analytics)
        .await
        .expect("bind")
        .expect("bound");
    assert!(kept.presented_session_kept);

    let unknown = SessionId::new(format!("ses_{}", Uuid::new_v4()));
    let fallback = rig
        .mint
        .bind_for_api_key(&rig.secret, Some(&unknown), &analytics)
        .await
        .expect("bind")
        .expect("bound");
    assert!(!fallback.presented_session_kept);
    assert_eq!(fallback.session_id, first.session_id);
}

#[tokio::test]
async fn an_anonymous_session_is_never_kept_for_an_api_key() {
    let rig = rig("bind-anonymous").await;
    let pool = test_db_pool().await;
    let analytics = SessionAnalytics::default();
    let bound = rig
        .mint
        .bind_for_api_key(&rig.secret, None, &analytics)
        .await
        .expect("bind")
        .expect("bound");
    let anonymous = SessionId::new(format!("sess_{}", Uuid::new_v4()));
    sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, session_source, expires_at) \
         VALUES ($1, NULL, 'bridge', now() + interval '1 hour')",
    )
    .bind(anonymous.as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .expect("seed an anonymous session");

    let presented = rig
        .mint
        .bind_for_api_key(&rig.secret, Some(&anonymous), &analytics)
        .await
        .expect("bind")
        .expect("bound");

    assert!(
        !presented.presented_session_kept,
        "a session with no owner is not the key owner's"
    );
    assert_eq!(presented.session_id, bound.session_id);
}
