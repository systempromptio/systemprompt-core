//! `security.allow_dynamic_client_registration: false`.
//!
//! With registration closed the `/register` endpoint refuses every caller and
//! discovery stops advertising a `registration_endpoint`, so a conforming MCP
//! client learns it must be pre-provisioned instead of failing at the POST.
//! The flag is read from the process-wide `Config`, which this binary installs
//! closed (see `middleware::security_trace_served_by`).

use axum::Extension;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use systemprompt_api::routes::oauth::{public_router, wellknown_routes};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::RequestContext;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

use crate::middleware::security_trace_served_by::ensure_config;

async fn ctx() -> std::sync::Arc<systemprompt_runtime::AppContext> {
    ensure_config();
    let url = fixture_database_url().expect("DATABASE_URL");
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("test database");
    fixture_app_context(&pool, &url).expect("app context")
}

#[tokio::test]
async fn register_is_forbidden_when_dcr_is_closed() {
    let ctx = ctx().await;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
    );
    let req_ctx = RequestContext::new(
        SessionId::generate(),
        TraceId::new("dcr-disabled"),
        ContextId::generate(),
        AgentName::system(),
    );
    let app = public_router().with_state(state).layer(Extension(req_ctx));
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
    let app = wellknown_routes(&ctx);
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
