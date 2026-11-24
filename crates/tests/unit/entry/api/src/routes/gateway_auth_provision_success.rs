use axum::body::Body;
use axum::extract::Request;
use axum::http::header;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, seed_bridge_credential, test_app_context, test_db_pool,
};

async fn harness() -> (AppContext, AuthedFixture) {
    let boot = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&boot.database_url).await;
    let user = seed_bridge_credential(&pool, "provision-ok")
        .await
        .expect("seed credential");
    ((*ctx).clone(), user)
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

fn request(user: &AuthedFixture, host: Option<&str>) -> Request {
    let mut builder = Request::builder()
        .uri("/v1/gateway/auth/oauth-client")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", user.jwt.as_str()),
        );
    if let Some(host) = host {
        builder = builder.header(header::HOST, host);
    }
    builder.body(Body::empty()).expect("request")
}

#[tokio::test]
async fn a_valid_bridge_jwt_provisions_a_client_with_the_token_endpoint() {
    let (ctx, user) = harness().await;
    let axum::Json(client) = provision_oauth_client(ctx, request(&user, None))
        .await
        .expect("provisioned");
    assert!(!client.client_id.as_str().is_empty());
    assert!(!client.client_secret.is_empty());
    assert!(
        client.token_endpoint.ends_with("/api/v1/core/oauth/token"),
        "{}",
        client.token_endpoint
    );
}

#[tokio::test]
async fn provisioning_twice_rotates_the_secret_of_one_client() {
    let (ctx, user) = harness().await;
    let axum::Json(first) = provision_oauth_client(ctx.clone(), request(&user, None))
        .await
        .expect("first");
    let axum::Json(second) = provision_oauth_client(ctx, request(&user, None))
        .await
        .expect("second");
    assert_eq!(first.client_id, second.client_id);
    assert_ne!(first.client_secret, second.client_secret);
    assert_eq!(first.token_endpoint, second.token_endpoint);
}

#[tokio::test]
async fn a_host_header_does_not_break_the_token_endpoint() {
    let (ctx, user) = harness().await;
    let axum::Json(client) =
        provision_oauth_client(ctx, request(&user, Some("gateway.example.invalid")))
            .await
            .expect("provisioned");
    assert!(
        client.token_endpoint.starts_with("http"),
        "{}",
        client.token_endpoint
    );
    assert!(client.token_endpoint.ends_with("/api/v1/core/oauth/token"));
}
