//! Static content answers before the composition's site fallback.
//!
//! With a site fallback admitted, a path static content serves is still
//! static's; a static miss and every non-read method reach the fallback; a
//! reserved path static misses is never the fallback's; the site-auth gate
//! covers the fallback; with no fallback the static miss is the answer.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::routing::get;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, ContextExtractor, McpContextMiddleware, MountLayers,
    ProtectionGuards, PublicContextMiddleware, RateLimitState, RootFallback, RootFallbackOwner,
    SiteAuthGuard, StaticMiss, UserOnlyContextMiddleware, protect_root_fallback,
    protect_site_fallback,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, RouteGroup, RouteProtection, SiteAuthConfig,
};
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{JwtAudience, JwtUserContext, RequestAuthenticator, UserType};
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_security::http::SessionCookie;
use systemprompt_test_fixtures::{fixture_config, test_db_pool};
use systemprompt_traits::AuthUser;
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;

#[derive(Clone)]
struct NoContext;

#[async_trait]
impl ContextExtractor for NoContext {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }
}

#[derive(Debug)]
struct NoCaller;

#[async_trait]
impl RequestAuthenticator for NoCaller {
    async fn authenticate(
        &self,
        _headers: &HeaderMap,
        _audiences: &[JwtAudience],
    ) -> Result<RequestContext, ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }

    async fn decode_for_audiences(
        &self,
        _token: &JwtToken,
        _audiences: &[JwtAudience],
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }

    async fn decode_for_gateway(
        &self,
        _token: &JwtToken,
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }

    async fn load_user(
        &self,
        _user_id: &UserId,
        _route_context: &str,
    ) -> Result<AuthUser, ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }
}

async fn layers(guards: ProtectionGuards) -> MountLayers {
    let config = fixture_config("postgres://localhost/static-first");
    let db = test_db_pool().await;
    MountLayers {
        limits: RateLimitState::new(&config, Arc::new(UserRateLimitBucketRepository::new(&db))),
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(NoContext),
        user_ui: UserOnlyContextMiddleware::new(NoContext),
        a2a: A2AContextMiddleware::new(NoContext, 1024, BodyBudget::new(64 * 1024 * 1024)),
        mcp: McpContextMiddleware::new(NoContext),
        bearer: None,
        guards,
        body_ceiling: systemprompt_manifest::profile::DEFAULT_MAX_ROUTE_BODY_BYTES,
    }
}

fn site_auth() -> ProtectionGuards {
    ProtectionGuards {
        site_auth: Some(SiteAuthGuard {
            config: SiteAuthConfig {
                login_path: "/login",
                protected_prefixes: &[],
                public_prefixes: &[],
                required_scope: "user",
            },
            authenticator: Arc::new(NoCaller),
            cookie: SessionCookie::for_external_url("http://localhost:8080"),
        }),
        client_writes: None,
    }
}

async fn homepage() -> &'static str {
    "static:home"
}

async fn docs() -> &'static str {
    "static:docs"
}

async fn missed() -> (StatusCode, axum::Extension<StaticMiss>, &'static str) {
    (
        StatusCode::NOT_FOUND,
        axum::Extension(StaticMiss),
        "static:miss",
    )
}

async fn site(request: Request) -> String {
    format!("site:{}", request.method())
}

fn statics(layers: &MountLayers) -> RootFallback {
    let protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public);
    let router = Router::new()
        .route("/", get(homepage))
        .route("/docs", get(docs))
        .fallback(missed);
    protect_root_fallback(router, &protection, layers, RouteGroup::Static)
        .expect("the static router mounts")
}

fn with_site_fallback(layers: &MountLayers) -> Router {
    let fallback = protect_site_fallback(
        Router::new().fallback_service(axum::routing::any(site).with_state(())),
        layers,
    )
    .expect("the site fallback mounts");
    RootFallbackOwner::default()
        .place(Router::new(), fallback.behind(statics(layers)))
        .expect("one fallback is placed")
}

async fn send(router: &Router, method: Method, uri: &str) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("valid request");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
    request.extensions_mut().insert(anonymous());
    let response = router.clone().oneshot(request).await.expect("infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a readable response");
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn anonymous() -> RequestContext {
    RequestContext::new(
        systemprompt_identifiers::SessionId::generate(),
        systemprompt_identifiers::TraceId::generate(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::AgentName::try_new("static-first").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(UserId::new("anonymous")),
    )
    .with_user_type(UserType::Anon)
}

#[tokio::test]
async fn a_static_hit_is_served_with_a_site_fallback_present() {
    let router = with_site_fallback(&layers(ProtectionGuards::default()).await);
    for (uri, expected) in [("/", "static:home"), ("/docs", "static:docs")] {
        let (status, _, body) = send(&router, Method::GET, uri).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, expected), "{uri}");
    }
}

#[tokio::test]
async fn a_static_miss_and_a_write_reach_the_site_fallback() {
    let router = with_site_fallback(&layers(ProtectionGuards::default()).await);
    let (status, _, body) = send(&router, Method::GET, "/app/board").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "site:GET"));
    let (status, _, body) = send(&router, Method::POST, "/docs").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "site:POST"),
        "static content is read-only, so a write is the fallback's"
    );
}

#[tokio::test]
async fn a_reserved_path_static_misses_is_never_the_fallbacks() {
    let router = with_site_fallback(&layers(ProtectionGuards::default()).await);
    for uri in [
        "/api/v1/core/contexts",
        "/.well-known/openid-configuration",
        "/v1/messages",
        "/api/v1/%61nalytics/stream",
    ] {
        let (status, _, body) = send(&router, Method::GET, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
        assert!(
            !body.starts_with("site:"),
            "{uri} is never the fallback's: {body}"
        );
    }
}

#[tokio::test]
async fn site_auth_still_covers_the_fallback() {
    let router = with_site_fallback(&layers(site_auth()).await);
    let (status, headers, body) = send(&router, Method::GET, "/app/board").await;
    assert!(status.is_redirection(), "{status} {body}");
    let location = headers
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(location.starts_with("/login?redirect="), "{location}");
    assert!(!body.starts_with("site:"), "{body}");
    let (status, _, body) = send(&router, Method::GET, "/login").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "site:GET"),
        "negative control: the login path passes the gate and reaches the fallback"
    );
}

#[tokio::test]
async fn without_a_site_fallback_the_static_miss_is_the_answer() {
    let layers = layers(ProtectionGuards::default()).await;
    let router = RootFallbackOwner::default()
        .place(Router::new(), statics(&layers))
        .expect("one fallback is placed");
    let (status, _, body) = send(&router, Method::GET, "/app/board").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::NOT_FOUND, "static:miss")
    );
    let (status, _, body) = send(&router, Method::GET, "/docs").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "static:docs"));
}
