// DB-backed bridge exchange-code tests (single-use, expiry).

use chrono::{Duration, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::{CreateExchangeCodeParams, OAuthRepository};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    user_id: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("xc");
    seed_user_row(&pool, &user_id, &format!("{}@xc.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    Some(Ctx { repo, user_id })
}

#[tokio::test]
async fn create_then_consume_once() {
    let Some(ctx) = setup().await else { return };
    let hash = format!("xc-{}", Uuid::new_v4());
    ctx.repo
        .create_bridge_exchange_code(CreateExchangeCodeParams {
            code_hash: &hash,
            user_id: &ctx.user_id,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect("create");

    let consumed = ctx
        .repo
        .consume_bridge_exchange_code(&hash)
        .await
        .expect("consume")
        .expect("present");
    assert_eq!(consumed, ctx.user_id);

    // Second consume yields None (single-use).
    assert!(
        ctx.repo
            .consume_bridge_exchange_code(&hash)
            .await
            .expect("consume again")
            .is_none()
    );
}

#[tokio::test]
async fn consume_unknown_returns_none() {
    let Some(ctx) = setup().await else { return };
    assert!(
        ctx.repo
            .consume_bridge_exchange_code(&format!("nope-{}", Uuid::new_v4()))
            .await
            .expect("consume")
            .is_none()
    );
}

#[tokio::test]
async fn expired_code_cannot_be_consumed() {
    let Some(ctx) = setup().await else { return };
    let hash = format!("xc-{}", Uuid::new_v4());
    ctx.repo
        .create_bridge_exchange_code(CreateExchangeCodeParams {
            code_hash: &hash,
            user_id: &ctx.user_id,
            expires_at: Utc::now() - Duration::minutes(1),
        })
        .await
        .expect("create");
    assert!(
        ctx.repo
            .consume_bridge_exchange_code(&hash)
            .await
            .expect("consume")
            .is_none()
    );
}
