//! Every request-context extractor the kernel mounts verifies through the one
//! request authenticator the users member provides, and the admission and
//! 401 behaviour of each context kind over it is the same as before: the
//! user and A2A flavours refuse a missing bearer with 401 and admit a valid
//! one, the MCP flavour admits a valid bearer, and the gateway decode refuses
//! an undecodable token.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::{Next, from_fn};
use axum::routing::any;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, JwtContextExtractor, McpContextMiddleware,
    UserOnlyContextMiddleware,
};
use systemprompt_extension::prelude::DependencyProvider;
use systemprompt_identifiers::JwtToken;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_test_fixtures::seed_admin_credential;
use tower::ServiceExt;

use super::common::setup_ctx;

fn probe(layer: impl FnOnce(Router) -> Router) -> Router {
    layer(Router::new().route(
        "/_probe",
        any(|req: Request| async move {
            req.extensions()
                .get::<RequestContext>()
                .map(|ctx| ctx.user_id().as_str().to_owned())
                .unwrap_or_default()
        }),
    ))
}

async fn status(app: Router, bearer: Option<&str>, body: &str) -> Result<(StatusCode, String)> {
    let mut req = Request::builder()
        .method("POST")
        .uri("/_probe")
        .header(header::HOST, "test.local");
    if let Some(token) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let resp = app.oneshot(req.body(Body::from(body.to_owned()))?).await?;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[tokio::test]
async fn the_mounted_extractor_verifies_through_the_users_members_authenticator() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let state = ctx.kernel().state();
    let seeded = state.get::<DynRequestAuthenticator>()?;
    let extractor = JwtContextExtractor::from_kernel(state)?;
    assert!(Arc::ptr_eq(extractor.authenticator(), seeded));
    assert!(
        matches!(
            state.provider_of::<DynRequestAuthenticator>(),
            Some(DependencyProvider::Extension(owner)) if owner.as_str() == "users"
        ),
        "the users member provides the one authenticator"
    );
    assert!(!state.is_sealed::<DynRequestAuthenticator>());
    Ok(())
}

#[tokio::test]
async fn the_user_flavour_refuses_a_missing_bearer_and_admits_a_valid_one() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let fixture = seed_admin_credential(&db, "auth-path-user").await?;
    let mw =
        UserOnlyContextMiddleware::new(JwtContextExtractor::from_kernel(ctx.kernel().state())?);
    let app = probe(move |r| {
        r.layer(from_fn(move |req: Request, next: Next| {
            let mw = mw.clone();
            async move { mw.handle(req, next).await }
        }))
    });
    assert_eq!(
        status(app.clone(), None, "").await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(app.clone(), Some("not.a.jwt"), "").await?.0,
        StatusCode::UNAUTHORIZED
    );
    let (ok, user) = status(app, Some(fixture.jwt.as_str()), "").await?;
    assert_eq!(ok, StatusCode::OK);
    assert_eq!(user, fixture.user_id.as_str());
    Ok(())
}

#[tokio::test]
async fn the_a2a_flavour_refuses_a_missing_bearer_and_admits_a_valid_one() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let fixture = seed_admin_credential(&db, "auth-path-a2a").await?;
    let mw = A2AContextMiddleware::new(
        JwtContextExtractor::from_kernel(ctx.kernel().state())?,
        64 * 1024,
        BodyBudget::new(1024 * 1024),
    );
    let app = probe(move |r| {
        r.layer(from_fn(move |req: Request, next: Next| {
            let mw = mw.clone();
            async move { mw.handle(req, next).await }
        }))
    });
    let body = serde_json::json!({
        "method": "message/send",
        "params": { "message": { "contextId": "44444444-4444-4444-8444-444444444444" } }
    })
    .to_string();
    assert_eq!(
        status(app.clone(), None, &body).await?.0,
        StatusCode::UNAUTHORIZED
    );
    let (ok, user) = status(app, Some(fixture.jwt.as_str()), &body).await?;
    assert_eq!(ok, StatusCode::OK);
    assert_eq!(user, fixture.user_id.as_str());
    Ok(())
}

#[tokio::test]
async fn the_mcp_flavour_admits_a_valid_bearer() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let fixture = seed_admin_credential(&db, "auth-path-mcp").await?;
    let mw = McpContextMiddleware::new(JwtContextExtractor::from_kernel(ctx.kernel().state())?);
    let app = probe(move |r| {
        r.layer(from_fn(move |req: Request, next: Next| {
            let mw = mw.clone();
            async move { mw.handle(req, next).await }
        }))
    });
    let (ok, user) = status(app, Some(fixture.jwt.as_str()), "").await?;
    assert_eq!(ok, StatusCode::OK);
    assert_eq!(user, fixture.user_id.as_str());
    Ok(())
}

#[tokio::test]
async fn the_gateway_decode_refuses_an_undecodable_token_and_admits_a_valid_one() -> Result<()> {
    let (db, ctx) = setup_ctx().await?;
    let fixture = seed_admin_credential(&db, "auth-path-gateway").await?;
    let authenticator = ctx
        .kernel()
        .state()
        .get::<DynRequestAuthenticator>()?
        .clone();
    assert!(
        authenticator
            .decode_for_gateway(&JwtToken::new("not.a.jwt"))
            .await
            .is_err()
    );
    let (claims, user) = authenticator
        .decode_for_gateway(&JwtToken::new(fixture.jwt.as_str().to_owned()))
        .await?;
    assert_eq!(claims.user_id.as_str(), fixture.user_id.as_str());
    assert_eq!(user.id.as_str(), fixture.user_id.as_str());
    Ok(())
}
