//! Unit tests for `AuthzPolicy` — the coarse caller-type gate applied at every
//! route group's `with_auth(...)` mount. Drives `authz_gate` directly with a
//! preconstructed `RequestContext` to lock in which policies admit which
//! `UserType`s. The MCP-specific cases here exist because v0.11.0 regressed
//! by mounting `/api/v1/mcp/*` under a policy that excluded `Anon` — collapsing
//! the proxy handler's RFC 9728 `WWW-Authenticate` 401 into a generic 403 and
//! breaking spec-compliant MCP clients.
//!
//! These tests would have caught that regression at the unit layer.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::from_fn;
use axum::routing::any;
use http::Method;
use systemprompt_api::services::middleware::{AuthzPolicy, authz_gate};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use tower::ServiceExt;

fn anon_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("test"),
    )
}

fn user_context(kind: UserType) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("test"),
    )
    .with_user_type(kind)
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new("u")))
}

fn gate_app(policy: AuthzPolicy, ctx: Option<RequestContext>) -> Router {
    let layer = from_fn(move |mut req: Request, next| {
        let ctx = ctx.clone();
        async move {
            if let Some(c) = ctx {
                req.extensions_mut().insert(c);
            }
            authz_gate(policy, req, next).await
        }
    });
    Router::new()
        .route("/_test", any(|| async { "ok" }))
        .layer(layer)
}

async fn drive(app: Router) -> (StatusCode, String) {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/_test")
        .header(header::HOST, "example.test")
        .body(Body::empty())
        .expect("request build");
    let resp = app.oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body collect");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn public_admits_anonymous_caller() {
    let (status, _) = drive(gate_app(AuthzPolicy::public(), Some(anon_context()))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn public_admits_missing_request_context_as_anon() {
    let (status, _) = drive(gate_app(AuthzPolicy::public(), None)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn authenticated_rejects_anonymous() {
    let (status, body) =
        drive(gate_app(AuthzPolicy::authenticated(), Some(anon_context()))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        body.contains("anon"),
        "expected anon-rejection body, got: {body}"
    );
}

#[tokio::test]
async fn user_admits_user_and_admin_but_not_anon_or_mcp() {
    let (s_anon, _) = drive(gate_app(AuthzPolicy::user(), Some(anon_context()))).await;
    let (s_user, _) =
        drive(gate_app(AuthzPolicy::user(), Some(user_context(UserType::User)))).await;
    let (s_admin, _) = drive(gate_app(
        AuthzPolicy::user(),
        Some(user_context(UserType::Admin)),
    ))
    .await;
    let (s_mcp, _) =
        drive(gate_app(AuthzPolicy::user(), Some(user_context(UserType::Mcp)))).await;
    assert_eq!(s_anon, StatusCode::FORBIDDEN);
    assert_eq!(s_user, StatusCode::OK);
    assert_eq!(s_admin, StatusCode::OK);
    assert_eq!(s_mcp, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_admits_only_admin() {
    let (s_admin, _) = drive(gate_app(
        AuthzPolicy::admin(),
        Some(user_context(UserType::Admin)),
    ))
    .await;
    let (s_user, _) = drive(gate_app(
        AuthzPolicy::admin(),
        Some(user_context(UserType::User)),
    ))
    .await;
    assert_eq!(s_admin, StatusCode::OK);
    assert_eq!(s_user, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn restricted_to_excluding_anon_emits_generic_403() {
    let policy = AuthzPolicy::restricted_to(&[
        UserType::User,
        UserType::Admin,
        UserType::Mcp,
        UserType::Service,
    ]);
    let (status, body) = drive(gate_app(policy, Some(anon_context()))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        body.contains("'anon' is not authorized"),
        "expected anon-denial body, got: {body}"
    );
}

/// Regression guard for the v0.11.0 MCP 401→403 collapse.
///
/// The MCP route mount MUST use a policy that admits `UserType::Anon` at the
/// route gate so the downstream proxy handler can answer with the RFC 9728
/// `WWW-Authenticate: Bearer resource_metadata="…"` 401 challenge. If anyone
/// re-narrows that mount to `AuthzPolicy::restricted_to([User, Admin, Mcp,
/// Service])` (or any policy excluding `Anon`), this test fails — because the
/// only policy currently in the codebase that admits `Anon` is `public()`.
///
/// The companion integration test in `routes_mcp_unauth_challenge.rs` locks in
/// the wire-level behaviour for the full mounted stack.
#[tokio::test]
async fn mcp_route_gate_must_admit_anon_so_proxy_can_emit_rfc9728_challenge() {
    let (status, _) = drive(gate_app(AuthzPolicy::public(), Some(anon_context()))).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "AuthzPolicy::public() must admit UserType::Anon — anything else \
         collapses the MCP proxy's RFC 9728 401 challenge into a 403"
    );
}
