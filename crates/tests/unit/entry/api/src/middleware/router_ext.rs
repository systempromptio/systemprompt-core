//! `RouterExt::with_rate_limit` / `with_auth` and the four `ContextLayer`
//! adapters.
//!
//! Nothing in the suite mounts either extension method, so the governor layer
//! is never built and no `ContextLayer::handle` adapter is ever entered — the
//! adapters are what every authenticated route group hangs off. The
//! rate-limiting cases are denial paths: an exhausted burst must refuse the
//! request rather than serve it, and a verified identity whose replica-shared
//! window is spent must be refused even when this replica has served nothing.
//! The limiter is built over the fixture database, so the suite skips when
//! none is reachable.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use chrono::{DateTime, Utc};
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, AuthzPolicy, ContextExtractor, McpContextMiddleware,
    PublicContextMiddleware, RateLimitState, RouterExt, UserOnlyContextMiddleware,
};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_models::{Config, RequestContext};
use systemprompt_test_fixtures::{fixture_config, fixture_database_url, fixture_db_pool};
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;
use uuid::Uuid;

const SCOPE: &str = "router-ext";
const WINDOW_SECS: i64 = 10;

fn ok_router() -> Router {
    Router::new().route("/", get(|| async { "ok" }))
}

fn request() -> Request<Body> {
    let mut req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("test request must build");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))));
    req
}

fn authed_request(ctx: RequestContext) -> Request<Body> {
    let mut req = request();
    req.extensions_mut().insert(ctx);
    req
}

fn context_for(user: &str) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("router-ext"),
    )
    .with_user_type(UserType::User)
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(user)))
}

fn context(kind: UserType) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("router-ext"),
    )
    .with_user_type(kind)
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
        "router-ext-user",
    )))
}

// The header-extracting flavours take a `ContextExtractor`; the trait itself is
// never implemented outside the crate, so a stub is the only way to drive both
// of its outcomes.
#[derive(Clone)]
struct StubExtractor {
    context: Option<RequestContext>,
}

#[async_trait::async_trait]
impl ContextExtractor for StubExtractor {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.context
            .clone()
            .ok_or(ContextExtractionError::MissingAuthHeader)
    }
}

fn extractor(context: Option<RequestContext>) -> StubExtractor {
    StubExtractor { context }
}

fn limited() -> RateLimitConfig {
    RateLimitConfig {
        disabled: false,
        burst_multiplier: 1,
        ..RateLimitConfig::default()
    }
}

fn config_with(rate_limits: RateLimitConfig) -> Config {
    let mut config = fixture_config("postgres://localhost/router-ext");
    config.rate_limits = rate_limits;
    config.trusted_proxies = Vec::new();
    config
}

async fn buckets_or_skip() -> Option<Arc<UserRateLimitBucketRepository>> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    Some(Arc::new(
        UserRateLimitBucketRepository::new(&db).expect("bucket repository"),
    ))
}

async fn limits_with_or_skip(rate_limits: RateLimitConfig) -> Option<RateLimitState> {
    Some(RateLimitState::new(
        &config_with(rate_limits),
        buckets_or_skip().await?,
    ))
}

fn window_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let secs = now.timestamp();
    DateTime::from_timestamp(secs - secs.rem_euclid(WINDOW_SECS), 0).expect("timestamp")
}

#[tokio::test]
async fn a_disabled_rate_limit_leaves_the_router_untouched() {
    let Some(limits) = limits_with_or_skip(RateLimitConfig {
        disabled: true,
        ..RateLimitConfig::default()
    })
    .await
    else {
        return;
    };
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a disabled limiter still builds");

    let resp = app.oneshot(request()).await.expect("request must complete");

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "with limiting off the layer must not be mounted at all"
    );
}

#[tokio::test]
async fn an_exhausted_burst_is_refused_rather_than_served() {
    let Some(limits) = limits_with_or_skip(limited()).await else {
        return;
    };
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a 1/s limiter must build");

    let mut statuses = Vec::new();
    for _ in 0..8 {
        statuses.push(
            app.clone()
                .oneshot(request())
                .await
                .expect("request must complete")
                .status(),
        );
    }

    assert_eq!(
        statuses[0],
        StatusCode::OK,
        "the first request is within the burst"
    );
    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "a burst of 8 against a 1/s quota must be refused somewhere: {statuses:?}"
    );
}

#[tokio::test]
async fn a_zero_rate_clamps_to_a_real_limit_instead_of_meaning_unlimited() {
    let Some(limits) = limits_with_or_skip(limited()).await else {
        return;
    };

    let app = ok_router()
        .with_rate_limit(&limits, 0, SCOPE)
        .expect("a zero rate clamps rather than failing to build");

    let mut statuses = Vec::new();
    for _ in 0..8 {
        statuses.push(
            app.clone()
                .oneshot(request())
                .await
                .expect("request must complete")
                .status(),
        );
    }

    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "a zero rate produced a zero burst, which the governor rejected and the router then \
         served unlimited: {statuses:?}"
    );
}

#[tokio::test]
async fn a_burst_product_that_is_an_exact_multiple_of_u32_still_limits() {
    let Some(limits) = limits_with_or_skip(RateLimitConfig {
        burst_multiplier: 1 << 31,
        ..limited()
    })
    .await
    else {
        return;
    };

    let app = ok_router()
        .with_rate_limit(&limits, 2, SCOPE)
        .expect("2 x 2^31 is exactly 2^32, which a truncating cast turns into a zero burst");

    let resp = app.oneshot(request()).await.expect("request must complete");

    assert_eq!(resp.status(), StatusCode::OK);
}

fn spoofed_request(forwarded_for: &str, user_agent: &str) -> Request<Body> {
    let mut req = Request::builder()
        .uri("/")
        .header("x-forwarded-for", forwarded_for)
        .header("user-agent", user_agent)
        .body(Body::empty())
        .expect("test request must build");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))));
    req
}

async fn statuses_over(app: &Router, requests: Vec<Request<Body>>) -> Vec<StatusCode> {
    let mut out = Vec::new();
    for req in requests {
        out.push(
            app.clone()
                .oneshot(req)
                .await
                .expect("request must complete")
                .status(),
        );
    }
    out
}

#[tokio::test]
async fn a_rotating_forwarded_for_header_does_not_mint_a_fresh_bucket() {
    let Some(limits) = limits_with_or_skip(limited()).await else {
        return;
    };
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a 1/s limiter must build");

    let requests = (0..8)
        .map(|i| spoofed_request(&format!("203.0.113.{i}"), "probe"))
        .collect();
    let statuses = statuses_over(&app, requests).await;

    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "no trusted proxy is configured, so a client-supplied X-Forwarded-For must not \
         choose the bucket: {statuses:?}"
    );
}

#[tokio::test]
async fn a_rotating_user_agent_does_not_mint_a_fresh_bucket() {
    let Some(limits) = limits_with_or_skip(limited()).await else {
        return;
    };
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a 1/s limiter must build");

    let requests = (0..8)
        .map(|i| spoofed_request("198.51.100.1", &format!("agent-{i}")))
        .collect();
    let statuses = statuses_over(&app, requests).await;

    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "an anonymous caller's identity is derived from the User-Agent, so rotating it \
         must not escape the limiter: {statuses:?}"
    );
}

#[tokio::test]
async fn two_authenticated_callers_get_independent_buckets() {
    let Some(limits) = limits_with_or_skip(limited()).await else {
        return;
    };
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a 1/s limiter must build");

    let mut first = Vec::new();
    for _ in 0..8 {
        first.push(authed_request(context_for("caller-one")));
    }
    let exhausted = statuses_over(&app, first).await;
    assert!(
        exhausted.contains(&StatusCode::TOO_MANY_REQUESTS),
        "the first caller must exhaust its own burst: {exhausted:?}"
    );

    let second = statuses_over(&app, vec![authed_request(context_for("caller-two"))]).await;
    assert_eq!(
        second[0],
        StatusCode::OK,
        "a different verified identity must not inherit another caller's spent budget"
    );
}

#[tokio::test]
async fn a_spent_replica_shared_window_refuses_a_verified_identity_this_replica_never_saw() {
    let Some(buckets) = buckets_or_skip().await else {
        return;
    };
    let limits = RateLimitState::new(&config_with(limited()), Arc::clone(&buckets));
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a 1/s limiter must build");
    let budget = WINDOW_SECS;

    // Why: the window is wall-clock aligned, so a preload that straddles a
    // boundary lands in a window the request never reads. Retry until both
    // fall in the same one rather than asserting on a torn window.
    let (status, retry_after) = loop {
        let user = format!("spent-{}", Uuid::new_v4().simple());
        let start = window_start(Utc::now());
        for _ in 0..budget {
            buckets
                .hit(&UserId::new(&user), SCOPE, start)
                .await
                .expect("preload hit");
        }
        let resp = app
            .clone()
            .oneshot(authed_request(context_for(&user)))
            .await
            .expect("request must complete");
        if window_start(Utc::now()) != start {
            continue;
        }
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok());
        break (resp.status(), retry_after);
    };

    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "hits recorded by other replicas must count against this caller's budget"
    );
    let retry_after = retry_after.expect("a global refusal carries Retry-After");
    assert!(
        (1..=WINDOW_SECS).contains(&retry_after),
        "Retry-After points at the end of the current window: {retry_after}"
    );

    let fresh = app
        .oneshot(authed_request(context_for(&format!(
            "fresh-{}",
            Uuid::new_v4().simple()
        ))))
        .await
        .expect("request must complete");
    assert_eq!(
        fresh.status(),
        StatusCode::OK,
        "a caller with an unspent window is served by the same router"
    );
}

#[tokio::test]
async fn an_anonymous_caller_never_touches_the_replica_shared_window() {
    let Some(buckets) = buckets_or_skip().await else {
        return;
    };
    let limits = RateLimitState::new(&config_with(limited()), Arc::clone(&buckets));
    let app = ok_router()
        .with_rate_limit(&limits, 1, SCOPE)
        .expect("a 1/s limiter must build");

    let status = app
        .oneshot(authed_request(context(UserType::Anon)))
        .await
        .expect("request must complete")
        .status();

    assert_eq!(
        status,
        StatusCode::OK,
        "an anonymous identity is derived from headers and must not be keyed globally"
    );
}

async fn drive_flavour(app: Router, ctx: Option<RequestContext>) -> StatusCode {
    let req = ctx.map_or_else(request, authed_request);
    app.oneshot(req)
        .await
        .expect("request must complete")
        .status()
}

#[tokio::test]
async fn every_context_flavour_refuses_a_request_with_no_session_context() {
    let public = ok_router().with_auth(PublicContextMiddleware::new(), AuthzPolicy::public());
    let user_only = ok_router().with_auth(
        UserOnlyContextMiddleware::new(extractor(None)),
        AuthzPolicy::public(),
    );
    let a2a = ok_router().with_auth(
        A2AContextMiddleware::new(extractor(None)),
        AuthzPolicy::public(),
    );
    let mcp = ok_router().with_auth(
        McpContextMiddleware::new(extractor(None)),
        AuthzPolicy::public(),
    );

    for (name, app) in [
        ("public", public),
        ("user-only", user_only),
        ("a2a", a2a),
        ("mcp", mcp),
    ] {
        let status = drive_flavour(app, None).await;
        assert_ne!(
            status,
            StatusCode::OK,
            "{name} must not serve a request that carries no session context"
        );
    }
}

#[tokio::test]
async fn the_public_flavour_admits_a_request_that_carries_its_context() {
    let app = ok_router().with_auth(PublicContextMiddleware::new(), AuthzPolicy::public());

    let status = drive_flavour(app, Some(context(UserType::User))).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "a request with a session context passes the public gate"
    );
}

#[tokio::test]
async fn the_policy_gate_runs_ahead_of_the_context_layer() {
    // `with_auth` mounts the authz gate outermost, so a caller type the policy
    // excludes is refused even though it carries a valid session context.
    let app = ok_router().with_auth(PublicContextMiddleware::new(), AuthzPolicy::admin());

    let status = drive_flavour(app, Some(context(UserType::Anon))).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_extracting_flavour_injects_the_context_it_resolved() {
    let resolved = context(UserType::Mcp);
    let app = ok_router().with_auth(
        McpContextMiddleware::new(extractor(Some(resolved))),
        AuthzPolicy::public(),
    );

    let status = drive_flavour(app, None).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "a request whose headers resolve to a context needs no pre-existing session"
    );
}

#[tokio::test]
async fn a_failed_extraction_falls_back_to_the_session_context() {
    let app = ok_router().with_auth(
        McpContextMiddleware::new(extractor(None)),
        AuthzPolicy::public(),
    );

    let status = drive_flavour(app, Some(context(UserType::User))).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "header extraction is an upgrade, not a requirement, when a session already exists"
    );
}

#[tokio::test]
async fn extract_from_request_defaults_to_the_header_extraction() {
    let resolved = context(UserType::A2a);
    let stub = extractor(Some(resolved.clone()));

    let (extracted, request) = stub
        .extract_from_request(request())
        .await
        .expect("the stub resolves a context");

    assert_eq!(
        extracted.user_id(),
        resolved.user_id(),
        "the default body must delegate to extract_from_headers"
    );
    assert_eq!(
        request.uri().path(),
        "/",
        "the request is handed back intact for the caller to forward"
    );
}

#[tokio::test]
async fn extract_from_request_propagates_an_extraction_failure() {
    let stub = extractor(None);

    let Err(err) = stub.extract_from_request(request()).await else {
        panic!("a stub with no context cannot resolve one");
    };

    assert!(
        matches!(err, ContextExtractionError::MissingAuthHeader),
        "the underlying reason must reach the caller: {err}"
    );
}
