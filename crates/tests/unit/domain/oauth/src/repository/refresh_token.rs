// DB-backed refresh-token persistence tests (store, validate, rotation,
// reuse-detection, family revocation, cleanup).

use chrono::{Duration, Utc};
use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenParams};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_oauth_client, seed_user_row,
    unique_user_id, OAuthClientFixture,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    client_id: ClientId,
    user_id: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("rt");
    seed_user_row(&pool, &user_id, &format!("{}@rt.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    let OAuthClientFixture { client_id, .. } =
        seed_oauth_client(&pool, &user_id).await.expect("seed client");
    Some(Ctx {
        repo,
        client_id,
        user_id,
    })
}

fn future_exp() -> i64 {
    (Utc::now() + Duration::hours(1)).timestamp()
}

async fn store(ctx: &Ctx, token: &RefreshTokenId, exp: i64) {
    ctx.repo
        .store_refresh_token(RefreshTokenParams {
            token_id: token,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            scope: "openid",
            expires_at: exp,
            family_id: None,
        })
        .await
        .expect("store refresh token");
}

#[tokio::test]
async fn store_then_validate() {
    let Some(ctx) = setup().await else { return };
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store(&ctx, &token, future_exp()).await;

    let (user, scope) = ctx
        .repo
        .validate_refresh_token(&token, &ctx.client_id)
        .await
        .expect("validate");
    assert_eq!(user, ctx.user_id);
    assert_eq!(scope, "openid");

    let cid = ctx
        .repo
        .get_client_id_from_refresh_token(&token)
        .await
        .expect("client from token")
        .expect("present");
    assert_eq!(cid, ctx.client_id);

    let family = ctx
        .repo
        .get_refresh_token_family(&token)
        .await
        .expect("family")
        .expect("present");
    assert!(!family.is_empty());
}

#[tokio::test]
async fn validate_unknown_token_errors() {
    let Some(ctx) = setup().await else { return };
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    assert!(ctx
        .repo
        .validate_refresh_token(&token, &ctx.client_id)
        .await
        .is_err());
    assert!(ctx
        .repo
        .get_refresh_token_family(&token)
        .await
        .expect("family")
        .is_none());
}

#[tokio::test]
async fn validate_expired_token_errors() {
    let Some(ctx) = setup().await else { return };
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    let past = (Utc::now() - Duration::hours(1)).timestamp();
    store(&ctx, &token, past).await;
    assert!(ctx
        .repo
        .validate_refresh_token(&token, &ctx.client_id)
        .await
        .is_err());
}

#[tokio::test]
async fn consume_then_replay_revokes_family() {
    let Some(ctx) = setup().await else { return };
    let exp = future_exp();
    let parent = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store(&ctx, &parent, exp).await;

    let family = ctx
        .repo
        .get_refresh_token_family(&parent)
        .await
        .expect("family")
        .expect("present");

    // A descendant carries the parent's family forward.
    let child = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    ctx.repo
        .store_refresh_token(RefreshTokenParams {
            token_id: &child,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            scope: "openid",
            expires_at: exp,
            family_id: Some(&family),
        })
        .await
        .expect("store child");

    let consumed = ctx
        .repo
        .consume_refresh_token(&parent, &ctx.client_id)
        .await
        .expect("consume");
    assert_eq!(consumed.user_id, ctx.user_id);
    assert_eq!(consumed.family_id, family);

    // Replaying the consumed parent revokes the whole family (parent + child).
    assert!(ctx
        .repo
        .consume_refresh_token(&parent, &ctx.client_id)
        .await
        .is_err());

    // Child is now gone too.
    assert!(ctx
        .repo
        .get_refresh_token_family(&child)
        .await
        .expect("child family")
        .is_none());
}

#[tokio::test]
async fn consume_unknown_token_errors() {
    let Some(ctx) = setup().await else { return };
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    assert!(ctx
        .repo
        .consume_refresh_token(&token, &ctx.client_id)
        .await
        .is_err());
}

#[tokio::test]
async fn revoke_refresh_token_deletes() {
    let Some(ctx) = setup().await else { return };
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store(&ctx, &token, future_exp()).await;

    assert!(ctx.repo.revoke_refresh_token(&token).await.expect("revoke"));
    assert!(!ctx
        .repo
        .revoke_refresh_token(&token)
        .await
        .expect("revoke again"));
    assert!(ctx
        .repo
        .validate_refresh_token(&token, &ctx.client_id)
        .await
        .is_err());
}

#[tokio::test]
async fn revoke_refresh_token_family_removes_all() {
    let Some(ctx) = setup().await else { return };
    let exp = future_exp();
    let a = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store(&ctx, &a, exp).await;
    let family = ctx
        .repo
        .get_refresh_token_family(&a)
        .await
        .expect("family")
        .expect("present");
    let b = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    ctx.repo
        .store_refresh_token(RefreshTokenParams {
            token_id: &b,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            scope: "openid",
            expires_at: exp,
            family_id: Some(&family),
        })
        .await
        .expect("store b");

    let removed = ctx
        .repo
        .revoke_refresh_token_family(&family)
        .await
        .expect("revoke family");
    assert!(removed >= 2);
}

#[tokio::test]
async fn cleanup_expired_refresh_tokens_removes_past() {
    let Some(ctx) = setup().await else { return };
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    let past = (Utc::now() - Duration::hours(2)).timestamp();
    store(&ctx, &token, past).await;
    let removed = ctx
        .repo
        .cleanup_expired_refresh_tokens()
        .await
        .expect("cleanup");
    assert!(removed >= 1);
}
