//! `POST /v1/gateway/auth/oauth-client` mints a per-user OAuth client for the
//! bridge, gated on a bridge JWT.
//!
//! Both refusals below must be 401. A missing header and an undecodable token
//! are the caller's problem, and answering 500 for either would tell a bridge
//! to retry against a server that is working perfectly.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use std::sync::Arc;
use systemprompt_api::routes::gateway::auth::provision_oauth_client;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use systemprompt_traits::AppContext as _;

async fn harness() -> (Arc<JwtContextExtractor>, AppContext) {
    let boot = ensure_test_bootstrap();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.analytics_provider().expect("analytics provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
    ));
    (extractor, (*ctx).clone())
}

async fn refuse(authorization: Option<&str>) -> StatusCode {
    let (extractor, ctx) = harness().await;
    let mut builder = Request::builder().uri("/v1/gateway/auth/oauth-client");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    let request = builder.body(Body::empty()).expect("request");

    provision_oauth_client(extractor, ctx, request)
        .await
        .map(|_| ())
        .expect_err("no client may be provisioned without a valid bridge JWT")
        .into_response()
        .status()
}

#[tokio::test]
async fn a_request_with_no_authorization_header_is_unauthorized() {
    assert_eq!(refuse(None).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_non_bearer_authorization_header_is_unauthorized() {
    assert_eq!(
        refuse(Some("Basic dXNlcjpwYXNz")).await,
        StatusCode::UNAUTHORIZED,
        "only a bearer credential is read here; any other scheme is no credential at all"
    );
}

#[tokio::test]
async fn a_bearer_token_that_is_not_a_jwt_is_unauthorized_not_a_server_error() {
    assert_eq!(
        refuse(Some("Bearer not-a-jwt")).await,
        StatusCode::UNAUTHORIZED,
        "an undecodable token is a rejected credential; a 500 would send the bridge into retries"
    );
}

#[tokio::test]
async fn a_well_formed_but_unsigned_jwt_is_unauthorized() {
    let forged = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiJ1c2VyIn0.";

    assert_eq!(
        refuse(Some(&format!("Bearer {forged}"))).await,
        StatusCode::UNAUTHORIZED,
        "an `alg: none` token carries no proof of identity and must never provision a client"
    );
}
