//! Tests for `inject_security_headers`, `inject_trace_header`, and
//! `inject_served_by` — all stateless `axum::middleware::from_fn` middlewares
//! that mutate the outgoing response.

use std::sync::Once;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::from_fn;
use axum::routing::get;
use systemprompt_api::services::middleware::{inject_security_headers, inject_trace_header};
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_models::{Config, RequestContext};
use tower::ServiceExt;

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let _ = Config::install(test_config());
    });
}

fn test_config() -> Config {
    Config {
        instance_id: "unit-test-instance".to_string(),
        max_concurrent_streams: 16,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: "postgres://x".to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: "/tmp".to_string(),
        geoip_database_path: None,
        web_path: "/tmp".to_string(),
        web_config_path: "/tmp".to_string(),
        web_metadata_path: "/tmp".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://127.0.0.1".to_string(),
        jwt_issuer: "test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: vec![],
        allowed_resource_audiences: vec![],
        trusted_issuers: vec![],
        id_jag_ttl_secs: 300,
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        use_https: false,
        rate_limits: RateLimitConfig::default(),
        cors_allowed_origins: vec![],
        trusted_proxies: vec![],
        is_cloud: false,
        system_admin_username: "admin".to_string(),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
    }
}

#[tokio::test]
async fn security_headers_inject_all_configured_headers() {
    let mut cfg = SecurityHeadersConfig::default();
    cfg.content_security_policy = Some("default-src 'self'".into());
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(move |req, next| {
            let cfg = cfg.clone();
            async move { inject_security_headers(cfg, req, next).await }
        }));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let h = resp.headers();
    assert!(h.contains_key("strict-transport-security"));
    assert!(h.contains_key("x-frame-options"));
    assert!(h.contains_key("x-content-type-options"));
    assert!(h.contains_key("referrer-policy"));
    assert!(h.contains_key("permissions-policy"));
    assert!(h.contains_key("content-security-policy"));
}

#[tokio::test]
async fn security_headers_skip_csp_when_none() {
    let cfg = SecurityHeadersConfig::default();
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(move |req, next| {
            let cfg = cfg.clone();
            async move { inject_security_headers(cfg, req, next).await }
        }));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(!resp.headers().contains_key("content-security-policy"));
}

fn security_app_with_frame_override(
    frame_options: systemprompt_extension::FrameOptions,
    cfg: SecurityHeadersConfig,
) -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(move |req, next| {
            systemprompt_extension::stamp_frame_options(frame_options, req, next)
        }))
        .layer(from_fn(move |req, next| {
            let cfg = cfg.clone();
            async move { inject_security_headers(cfg, req, next).await }
        }))
}

#[tokio::test]
async fn frame_override_allow_all_removes_xfo_and_sets_frame_ancestors() {
    let app = security_app_with_frame_override(
        systemprompt_extension::FrameOptions::AllowAll,
        SecurityHeadersConfig::default(),
    );
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let h = resp.headers();
    assert!(!h.contains_key("x-frame-options"));
    assert_eq!(
        h.get("content-security-policy")
            .and_then(|v| v.to_str().ok()),
        Some("frame-ancestors *")
    );
}

#[tokio::test]
async fn frame_override_same_origin_sets_xfo_and_frame_ancestors() {
    let app = security_app_with_frame_override(
        systemprompt_extension::FrameOptions::SameOrigin,
        SecurityHeadersConfig::default(),
    );
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let h = resp.headers();
    assert_eq!(
        h.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("SAMEORIGIN")
    );
    assert_eq!(
        h.get("content-security-policy")
            .and_then(|v| v.to_str().ok()),
        Some("frame-ancestors 'self'")
    );
}

#[tokio::test]
async fn frame_override_replaces_global_csp() {
    let mut cfg = SecurityHeadersConfig::default();
    cfg.content_security_policy = Some("default-src 'self'".into());
    let app = security_app_with_frame_override(systemprompt_extension::FrameOptions::AllowAll, cfg);
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        resp.headers()
            .get("content-security-policy")
            .and_then(|v| v.to_str().ok()),
        Some("frame-ancestors *")
    );
}

#[tokio::test]
async fn raw_xfo_header_without_marker_is_clobbered_to_profile_value() {
    let cfg = SecurityHeadersConfig::default();
    let app = Router::new()
        .route(
            "/",
            get(|| async { ([("x-frame-options", "ALLOWALL")], "ok") }),
        )
        .layer(from_fn(move |req, next| {
            let cfg = cfg.clone();
            async move { inject_security_headers(cfg, req, next).await }
        }));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        resp.headers()
            .get("x-frame-options")
            .and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
}

#[tokio::test]
async fn trace_header_present_when_context_attached() {
    use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
    let trace = TraceId::new("trace-xyz");
    let ctx = RequestContext::new(
        SessionId::generate(),
        trace.clone(),
        ContextId::generate(),
        AgentName::new("agent"),
    );
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(inject_trace_header));
    let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
    req.extensions_mut().insert(ctx);
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.headers()
            .get("x-trace-id")
            .and_then(|h| h.to_str().ok()),
        Some("trace-xyz")
    );
}

#[tokio::test]
async fn trace_header_absent_without_context() {
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(inject_trace_header));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(!resp.headers().contains_key("x-trace-id"));
}

#[tokio::test]
async fn served_by_injects_instance_id_when_config_present() {
    ensure_config();
    use systemprompt_api::services::middleware::inject_served_by;
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn(inject_served_by));
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let val = resp
        .headers()
        .get("x-served-by")
        .and_then(|h| h.to_str().ok());
    assert_eq!(val, Some("unit-test-instance"));
}
