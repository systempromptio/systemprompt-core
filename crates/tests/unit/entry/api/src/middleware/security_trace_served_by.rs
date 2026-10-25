//! Tests for `inject_security_headers`, `inject_trace_header`, and
//! `inject_served_by` — all stateless `axum::middleware::from_fn` middlewares
//! that mutate the outgoing response.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::get;
use systemprompt_api::services::middleware::{inject_security_headers, inject_trace_header};
use systemprompt_manifest::profile::SecurityHeadersConfig;
use systemprompt_models::RequestContext;
use tower::ServiceExt;

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
    frame_options: systemprompt_manifest::profile::FrameOptions,
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
        systemprompt_manifest::profile::FrameOptions::AllowAll,
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
        systemprompt_manifest::profile::FrameOptions::SameOrigin,
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
    let app = security_app_with_frame_override(
        systemprompt_manifest::profile::FrameOptions::AllowAll,
        cfg,
    );
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
    use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
    let trace = TraceId::new("trace-xyz");
    let ctx = RequestContext::new(
        SessionId::generate(),
        trace.clone(),
        ContextId::generate(),
        AgentName::try_new("agent").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
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
    use systemprompt_api::services::middleware::inject_served_by;
    let app = Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(from_fn_with_state(
            systemprompt_identifiers::InstanceId::new("unit-test-instance"),
            inject_served_by,
        ));
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
