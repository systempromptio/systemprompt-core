// DB-backed refresh-token persistence tests (store, consume, rotation,
// reuse-detection and family revocation through the consume path).

use chrono::{Duration, Utc};
use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenParams};
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, seed_oauth_client, seed_user_row, test_db_pool,
    unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    client_id: ClientId,
    user_id: UserId,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);
    let user_id = unique_user_id("rt");
    seed_user_row(&pool, &user_id, &format!("{}@rt.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    let OAuthClientFixture { client_id, .. } = seed_oauth_client(&pool, &user_id)
        .await
        .expect("seed client");
    Ctx {
        repo,
        client_id,
        user_id,
    }
}

fn future_exp() -> i64 {
    (Utc::now() + Duration::hours(1)).timestamp()
}

async fn store_in_family(ctx: &Ctx, token: &RefreshTokenId, exp: i64, family: Option<&str>) {
    ctx.repo
        .store_refresh_token(RefreshTokenParams {
            token_id: token,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            scope: "openid",
            expires_at: exp,
            family_id: family,
        })
        .await
        .expect("store refresh token");
}

async fn store(ctx: &Ctx, token: &RefreshTokenId, exp: i64) {
    store_in_family(ctx, token, exp, None).await;
}

#[tokio::test]
async fn store_then_consume() {
    let ctx = setup().await;
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store(&ctx, &token, future_exp()).await;

    let cid = ctx
        .repo
        .find_client_id_from_refresh_token(&token)
        .await
        .expect("client from token")
        .expect("present");
    assert_eq!(cid, ctx.client_id);

    let consumed = ctx
        .repo
        .consume_refresh_token(&token, &ctx.client_id)
        .await
        .expect("consume");
    assert_eq!(consumed.user_id, ctx.user_id);
    assert!(!consumed.family_id.is_empty());
}

#[tokio::test]
async fn consume_unknown_token_errors() {
    let ctx = setup().await;
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    assert!(
        ctx.repo
            .consume_refresh_token(&token, &ctx.client_id)
            .await
            .is_err()
    );
    assert!(
        ctx.repo
            .find_client_id_from_refresh_token(&token)
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn consume_then_replay_revokes_family() {
    let ctx = setup().await;
    let exp = future_exp();
    let family = format!("family-{}", Uuid::new_v4());
    let parent = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    let child = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store_in_family(&ctx, &parent, exp, Some(&family)).await;
    store_in_family(&ctx, &child, exp, Some(&family)).await;

    let consumed = ctx
        .repo
        .consume_refresh_token(&parent, &ctx.client_id)
        .await
        .expect("consume");
    assert_eq!(consumed.user_id, ctx.user_id);
    assert_eq!(consumed.family_id, family);

    assert!(
        ctx.repo
            .consume_refresh_token(&parent, &ctx.client_id)
            .await
            .is_err(),
        "replaying the consumed parent must fail"
    );
    assert!(
        ctx.repo
            .consume_refresh_token(&child, &ctx.client_id)
            .await
            .is_err(),
        "the replay revokes every token in the family"
    );
}

#[tokio::test]
async fn revoke_refresh_token_deletes() {
    let ctx = setup().await;
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    store(&ctx, &token, future_exp()).await;

    assert!(ctx.repo.revoke_refresh_token(&token).await.expect("revoke"));
    assert!(
        !ctx.repo
            .revoke_refresh_token(&token)
            .await
            .expect("revoke again")
    );
    assert!(
        ctx.repo
            .consume_refresh_token(&token, &ctx.client_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn consume_expired_unconsumed_token_reports_expired() {
    let ctx = setup().await;
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    let past = (Utc::now() - Duration::hours(2)).timestamp();
    store(&ctx, &token, past).await;

    let err = ctx
        .repo
        .consume_refresh_token(&token, &ctx.client_id)
        .await
        .expect_err("expired token cannot be consumed");
    assert!(
        err.to_string().contains("expired"),
        "expected expiry error, got {err}"
    );
}
