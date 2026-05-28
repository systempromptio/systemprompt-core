//! Unit tests for the four sibling context middlewares
//! ([`PublicContextMiddleware`], [`UserOnlyContextMiddleware`],
//! [`A2AContextMiddleware`], [`McpContextMiddleware`]).
//!
//! Each flavour encodes its own caller-admission contract at the type level.
//! These tests pin those contracts so a future "let's collapse them again"
//! refactor cannot silently re-introduce the v0.11.0 regression (where the
//! coarse public-flavour gate ate the MCP proxy's RFC 9728 401 challenge).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{Next, from_fn};
use axum::routing::any;
use http::Method;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, ContextExtractor, McpContextMiddleware, PublicContextMiddleware,
    UserOnlyContextMiddleware,
};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::ContextExtractionError;
use tower::ServiceExt;

fn anon_session_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("session"),
    )
}

fn real_user_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("real"),
    )
    .with_user_type(UserType::User)
    .with_actor(Actor::user(UserId::new("u-1")))
}

struct OkExtractor {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ContextExtractor for OkExtractor {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(real_user_context())
    }
}

struct FailExtractor;

#[async_trait]
impl ContextExtractor for FailExtractor {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }
}

fn pipeline_with_session<F>(layer_factory: F, session: Option<RequestContext>) -> Router
where
    F: FnOnce(Router) -> Router,
{
    let seed = from_fn(move |mut req: Request, next: Next| {
        let session = session.clone();
        async move {
            if let Some(s) = session {
                req.extensions_mut().insert(s);
            }
            next.run(req).await
        }
    });

    let app = Router::new().route(
        "/_probe",
        any(|req: Request| async move {
            let ctx = req
                .extensions()
                .get::<RequestContext>()
                .cloned()
                .expect("handler must see a RequestContext");
            format!(
                "agent={};context_id={};user_type={:?}",
                ctx.execution.agent_name,
                ctx.execution.context_id,
                ctx.user_type(),
            )
        }),
    );
    layer_factory(app).layer(seed)
}

async fn drive(app: Router, method: Method, headers: &[(&str, &str)]) -> (StatusCode, String) {
    let mut req = Request::builder()
        .method(method)
        .uri("/_probe")
        .header(header::HOST, "test.local");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = app
        .oneshot(req.body(Body::empty()).expect("request build"))
        .await
        .expect("oneshot");
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body collect");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn public_forwards_session_anon_context_unchanged() {
    let mw = PublicContextMiddleware::new();
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, body) = drive(app, Method::GET, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("agent=session;"),
        "public flavour must forward the session-derived context, got {body}"
    );
}

#[tokio::test]
async fn public_merges_optional_context_id_and_agent_name_headers() {
    let mw = PublicContextMiddleware::new();
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, body) = drive(
        app,
        Method::GET,
        &[
            ("x-context-id", "ctx-from-header"),
            ("x-agent-name", "claude-code"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("context_id=ctx-from-header"),
        "x-context-id header must override session context_id, got {body}"
    );
    assert!(
        body.contains("agent=claude-code;"),
        "x-agent-name header must override session agent, got {body}"
    );
}

#[tokio::test]
async fn public_500s_when_session_middleware_did_not_run() {
    let mw = PublicContextMiddleware::new();
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        None,
    );
    let (status, _) = drive(app, Method::GET, &[]).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn user_only_admits_when_extractor_succeeds() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mw = UserOnlyContextMiddleware::new(OkExtractor {
        calls: Arc::clone(&calls),
    });
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, body) = drive(app, Method::GET, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        body.contains("user_type=User"),
        "user-only flavour must replace session ctx with the extractor's user, got {body}"
    );
}

#[tokio::test]
async fn user_only_rejects_when_extractor_fails() {
    let mw = UserOnlyContextMiddleware::new(FailExtractor);
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, body) = drive(app, Method::GET, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        body.contains("Missing Authorization header"),
        "user-only flavour must surface extractor's auth error, got {body}"
    );
}

#[tokio::test]
async fn a2a_admits_when_extractor_succeeds_and_runs_extract_from_request() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mw = A2AContextMiddleware::new(OkExtractor {
        calls: Arc::clone(&calls),
    });
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, _) = drive(app, Method::POST, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn mcp_falls_back_to_session_context_on_extractor_failure() {
    let mw = McpContextMiddleware::new(FailExtractor);
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, body) = drive(app, Method::POST, &[]).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "MCP flavour must fall through with the session Anon context on extractor \
         failure so the downstream proxy can emit an RFC 9728 401 challenge — \
         see routes_mcp_unauth_challenge integration test"
    );
    assert!(
        body.contains("agent=session;"),
        "MCP fallback must use the session-derived context, got {body}"
    );
}

#[tokio::test]
async fn mcp_500s_when_extractor_fails_and_no_session_context_present() {
    let mw = McpContextMiddleware::new(FailExtractor);
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        None,
    );
    let (status, _) = drive(app, Method::POST, &[]).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn mcp_admits_when_extractor_succeeds() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mw = McpContextMiddleware::new(OkExtractor {
        calls: Arc::clone(&calls),
    });
    let app = pipeline_with_session(
        move |r| {
            r.layer(from_fn(move |req, next| {
                let mw = mw.clone();
                async move { mw.handle(req, next).await }
            }))
        },
        Some(anon_session_context()),
    );
    let (status, body) = drive(app, Method::POST, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        body.contains("user_type=User"),
        "MCP flavour must use the extractor's context on success, got {body}"
    );
}
