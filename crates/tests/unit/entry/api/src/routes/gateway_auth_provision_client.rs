//! `POST /v1/gateway/auth/oauth-client` mints a per-user OAuth client for the
//! bridge, gated on a bridge JWT.
//!
//! Both refusals below must be 401. A missing header and an undecodable token
//! are the caller's problem, and answering 500 for either would tell a bridge
//! to retry against a server that is working perfectly.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context};

async fn harness() -> AppContext {
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&boot.database_url).await;
    (*ctx).clone()
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Provisioned {
    client_id: String,
    client_secret: String,
    token_endpoint: String,
}

async fn provision_oauth_client(
    ctx: AppContext,
    request: Request,
) -> Result<axum::Json<Provisioned>, axum::response::Response> {
    use tower::ServiceExt;
    let (mut parts, body) = request.into_parts();
    parts.uri = systemprompt_models::ApiPaths::gateway_relative(
        systemprompt_models::ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT,
    )
    .parse()
    .expect("uri");
    parts.method = axum::http::Method::POST;
    let app = systemprompt_api::routes::gateway::gateway_router(&ctx, ctx.admitted(), ctx.kernel())
        .expect("gateway router");
    let response = app
        .oneshot(Request::from_parts(parts, body))
        .await
        .expect("the provisioning route answers");
    if !response.status().is_success() {
        return Err(response);
    }
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    Ok(axum::Json(
        serde_json::from_slice(&bytes).expect("a provisioned client"),
    ))
}

async fn refuse(authorization: Option<&str>) -> StatusCode {
    let ctx = harness().await;
    let mut builder = Request::builder().uri("/v1/gateway/auth/oauth-client");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    let request = builder.body(Body::empty()).expect("request");

    provision_oauth_client(ctx, request)
        .await
        .map(|_| ())
        .expect_err("no client may be provisioned without a valid bridge JWT")
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
