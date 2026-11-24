//! `security.allow_dynamic_client_registration: false`.
//!
//! With registration closed the `/register` endpoint refuses every caller and
//! discovery stops advertising a `registration_endpoint`, so a conforming MCP
//! client learns it must be pre-provisioned instead of failing at the POST.
//! The flag is read from the `Config` each test's context is built with.

use std::sync::Arc;

use axum::Extension;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_identity_entry::http::oauth::{public_router, wellknown_routes};
use systemprompt_models::RequestContext;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth_issuance::JwksCache;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_database_url,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

async fn ctx() -> std::sync::Arc<systemprompt_runtime::AppContext> {
    let url = test_database_url();
    ensure_test_bootstrap();
    let mut config = fixture_config(&url);
    config.allow_dynamic_client_registration = false;
    fixture_app_context_with_config(config)
        .await
        .expect("fixture AppContext with DCR closed")
}

#[tokio::test]
async fn register_is_forbidden_when_dcr_is_closed() {
    let ctx = ctx().await;
    let state = OAuthState::new(
        systemprompt_test_fixtures::oauth_repositories(&ctx)
            .oauth
            .clone(),
        ctx.analytics_provider(),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    let req_ctx = RequestContext::new(
        SessionId::generate(),
        TraceId::new("dcr-disabled"),
        ContextId::generate(),
        AgentName::system(),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    );
    let app = public_router(Arc::new(JwksCache::new()))
        .with_state(state)
        .layer(Extension(req_ctx))
        .layer(Extension(ctx.config_arc().clone()));
    let body = serde_json::json!({
        "client_name": "probe",
        "redirect_uris": ["https://app.example/cb"],
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.expect("body");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(v["error"].as_str(), Some("access_denied"), "{v}");
}

#[tokio::test]
async fn discovery_omits_registration_endpoint_when_dcr_is_closed() {
    let ctx = ctx().await;
    let app = wellknown_routes(
        systemprompt_identity_entry::http::oauth::discovery::DiscoveryState::from_config(
            ctx.config(),
            ctx.kernel()
                .state()
                .find::<std::sync::Arc<dyn systemprompt_traits::McpRegistryProvider>>()
                .cloned(),
        ),
    )
    .layer(Extension(ctx.config_arc().clone()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri(ApiPaths::WELLKNOWN_OAUTH_SERVER)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.expect("body");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(
        v.get("registration_endpoint").is_none(),
        "closed DCR must not be advertised: {v}"
    );
    assert!(v["token_endpoint"].as_str().is_some(), "{v}");
}
