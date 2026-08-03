//! A valid signature is not enough: the user behind a JWT must still exist and
//! still be active.
//!
//! `validate_user` and `require_active` guard every authenticated route, and
//! their denial arms are never taken by the suite — every existing test mints a
//! token for a user who stays present and enabled for the whole test. A token
//! that outlives its account is exactly the case that must not be honoured, so
//! these tests mint a real credential and then remove or disable the account
//! behind it.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, fixture_app_context, fixture_db_pool,
    install_test_signing_key, seed_admin_credential,
};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::body_to_string;

async fn app() -> Result<(Router, DbPool)> {
    let b = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok((
        gateway_router(&ctx).expect("gateway router available"),
        pool,
    ))
}

async fn credential(pool: &DbPool) -> Result<AuthedFixture> {
    seed_admin_credential(
        pool,
        &format!("jwt-state-{}@example.invalid", Uuid::new_v4()),
    )
    .await
}

fn whoami(token: &str) -> Request<Body> {
    Request::builder()
        .uri("/bridge/whoami")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request must build")
}

async fn deactivate(pool: &DbPool, user: &UserId) -> Result<()> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(user.as_str())
        .execute(pg.as_ref())
        .await?;
    Ok(())
}

async fn delete_user(pool: &DbPool, user: &UserId) -> Result<()> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.as_str())
        .execute(pg.as_ref())
        .await?;
    Ok(())
}

#[tokio::test]
async fn a_valid_token_for_a_live_active_user_is_honoured() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;

    let (status, body) = body_to_string(app.oneshot(whoami(cred.jwt.as_str())).await?).await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains(cred.user_id.as_str()), "{body}");
    Ok(())
}

#[tokio::test]
async fn a_token_whose_user_has_been_deactivated_is_refused() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;
    deactivate(&pool, &cred.user_id).await?;

    let (status, body) = body_to_string(app.oneshot(whoami(cred.jwt.as_str())).await?).await?;

    // The signature is still perfectly valid — disabling an account has to take
    // effect without waiting for the token to expire.
    assert!(
        status.is_client_error(),
        "a deactivated account must not keep serving its outstanding tokens, got {status}: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_token_whose_user_has_been_deleted_is_refused() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = credential(&pool).await?;
    delete_user(&pool, &cred.user_id).await?;

    let (status, body) = body_to_string(app.oneshot(whoami(cred.jwt.as_str())).await?).await?;

    assert!(
        status.is_client_error(),
        "a token for an account that no longer exists must be refused, got {status}: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn deactivating_one_user_does_not_lock_out_another() -> Result<()> {
    let (app, pool) = app().await?;
    let victim = credential(&pool).await?;
    let bystander = credential(&pool).await?;
    deactivate(&pool, &victim.user_id).await?;

    let (status, body) = body_to_string(app.oneshot(whoami(bystander.jwt.as_str())).await?).await?;

    assert_eq!(
        status.as_u16(),
        200,
        "an unrelated account must be unaffected: {body}"
    );
    Ok(())
}
