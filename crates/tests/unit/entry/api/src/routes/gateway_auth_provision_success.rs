use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::http::header;
use systemprompt_api::routes::gateway::auth::provision_oauth_client;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, seed_bridge_credential, test_app_context, test_db_pool,
};
use systemprompt_traits::AppContext as _;

async fn harness() -> (Arc<JwtContextExtractor>, AppContext, AuthedFixture) {
    let boot = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &boot.database_url);
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(
            ctx.oauth_repositories().oauth.clone(),
            systemprompt_traits::CacheEpochs::new(),
        ),
        ctx.config().jwt_issuer.clone(),
        systemprompt_test_fixtures::test_signing_keys(),
    ));
    let user = seed_bridge_credential(&pool, "provision-ok")
        .await
        .expect("seed credential");
    (extractor, (*ctx).clone(), user)
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
    let (extractor, ctx, user) = harness().await;
    let axum::Json(client) = provision_oauth_client(extractor, ctx, request(&user, None))
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
    let (extractor, ctx, user) = harness().await;
    let axum::Json(first) =
        provision_oauth_client(Arc::clone(&extractor), ctx.clone(), request(&user, None))
            .await
            .expect("first");
    let axum::Json(second) = provision_oauth_client(extractor, ctx, request(&user, None))
        .await
        .expect("second");
    assert_eq!(first.client_id, second.client_id);
    assert_ne!(first.client_secret, second.client_secret);
    assert_eq!(first.token_endpoint, second.token_endpoint);
}

#[tokio::test]
async fn a_host_header_does_not_break_the_token_endpoint() {
    let (extractor, ctx, user) = harness().await;
    let axum::Json(client) = provision_oauth_client(
        extractor,
        ctx,
        request(&user, Some("gateway.example.invalid")),
    )
    .await
    .expect("provisioned");
    assert!(
        client.token_endpoint.starts_with("http"),
        "{}",
        client.token_endpoint
    );
    assert!(client.token_endpoint.ends_with("/api/v1/core/oauth/token"));
}
