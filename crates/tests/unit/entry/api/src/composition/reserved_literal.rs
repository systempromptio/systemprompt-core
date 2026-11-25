//! Reserved literals beside install routes, a route's own byte limit, and the
//! site-fallback slot, on the enumerating mount over forged handoffs.
//!
//! An install parameter route beside a reserved literal mounts; the reserved
//! route still answers its own requests under its own policy (method
//! variants included), and the install route answers `404` for a reserved
//! path the router hands it (the owner not composed, an encoded spelling).
//! A route's own byte limit holds smaller and larger than the default on a
//! streamed body, and one above the profile ceiling refuses the mount naming
//! the route. The site fallback answers unmatched paths publicly and never a
//! reserved one.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::routing::get;
use futures_util::stream;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, ContextExtractor, McpContextMiddleware, MountError,
    MountLayers, ProtectionGuards, PublicContextMiddleware, RateLimitState, RootFallbackOwner,
    UserOnlyContextMiddleware, mount_admitted, protect_site_fallback, site_fallback_protection,
};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec, SiteFallback, WebBase,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandleId, RateBudget, RateLimit, RouteGroup, StandingTag,
};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::{fixture_config, test_db_pool};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;

use super::forge::{ProbeState, analytics_set, forged, ok, routed};

const CEILING: usize = 4 * 1024 * 1024;
const ROOT: BasePath = BasePath::Web(WebBase::Root);
const BODY_BASE: BasePath = BasePath::Api(ApiBase::Extension("/api/v1/probe-body"));

#[derive(Clone)]
struct NoContext;

#[async_trait::async_trait]
impl ContextExtractor for NoContext {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }
}

#[derive(Debug)]
struct StubDb;

impl DatabaseHandle for StubDb {
    fn is_connected(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

async fn layers() -> MountLayers {
    let config = fixture_config("postgres://localhost/reserved-literal");
    let db = test_db_pool().await;
    MountLayers {
        limits: RateLimitState::new(&config, Arc::new(UserRateLimitBucketRepository::new(&db))),
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(NoContext),
        user_ui: UserOnlyContextMiddleware::new(NoContext),
        a2a: A2AContextMiddleware::new(NoContext, 1024, BodyBudget::new(64 * 1024 * 1024)),
        mcp: McpContextMiddleware::new(NoContext),
        bearer: None,
        guards: ProtectionGuards::default(),
        body_ceiling: CEILING,
    }
}

fn kernel() -> Kernel {
    Kernel::new(
        Arc::new(StubDb),
        BackgroundTasks::new(),
        Dependencies::default(),
    )
}

fn public_spec(body: BodyLimit) -> RouteSpec {
    RouteSpec::new(
        AuthzPolicy::public(),
        ContextKind::Public,
        RouteLimits {
            rate: RateLimit::new("probe", RateBudget::PerSecond(10_000)),
            body,
        },
    )
}

fn install(set: RouteSet) -> AdmittedRouteSet {
    AdmittedRouteSet {
        standing: StandingTag::Install,
        ..routed("probe-install", set, vec![])
    }
}

fn reserved_owner() -> AdmittedRouteSet {
    routed(
        "analytics-http",
        analytics_set(),
        vec![HandleId::AnalyticsBroadcaster],
    )
}

async fn mounted(routes: Vec<AdmittedRouteSet>) -> Result<Router, MountError> {
    let admitted = forged(routes, vec![]);
    mount_admitted(
        Router::new(),
        Composed {
            admitted: &admitted,
            kernel: &kernel(),
        },
        &layers().await,
        &|_| true,
        &|router| router,
    )
}

async fn send(router: Router, method: Method, uri: &str, body: Body) -> (StatusCode, String) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer forged")
        .body(body)
        .expect("valid request");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
    request.extensions_mut().insert(anonymous());
    let response = router.oneshot(request).await.expect("infallible");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a readable response");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_status(router: &Router, uri: &str) -> (StatusCode, String) {
    send(router.clone(), Method::GET, uri, Body::empty()).await
}

fn anonymous() -> RequestContext {
    RequestContext::new(
        systemprompt_identifiers::SessionId::generate(),
        systemprompt_identifiers::TraceId::generate(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::AgentName::try_new("reserved-literal").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(systemprompt_identifiers::UserId::new("anonymous")),
    )
    .with_user_type(UserType::Anon)
}

async fn install_page() -> &'static str {
    "install-page"
}

fn short_links() -> RouteSet {
    RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
        .route(
            Method::GET,
            "/api/v1/{short_code}",
            install_page,
            public_spec(BodyLimit::Default),
        )
        .build()
}

#[tokio::test]
async fn a_reserved_route_answers_under_its_own_policy_beside_an_install_parameter() {
    let router = mounted(vec![reserved_owner(), install(short_links())])
        .await
        .expect("an install parameter beside a reserved literal mounts");

    let (owned, body) = get_status(&router, "/api/v1/analytics/stream").await;
    assert!(
        owned == StatusCode::UNAUTHORIZED || owned == StatusCode::FORBIDDEN,
        "the reserved route keeps its admin policy, not the install's public one: {owned} {body}"
    );
    let (method, body) = send(
        router.clone(),
        Method::POST,
        "/api/v1/analytics/stream",
        Body::empty(),
    )
    .await;
    assert!(
        matches!(
            method,
            StatusCode::METHOD_NOT_ALLOWED | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) && body != "install-page",
        "a method variant is answered by the reserved route under its own gate: {method} {body}"
    );
    for uri in [
        "/api/v1/analytics",
        "/api/v1/%61nalytics",
        "/api/v1/analytics/",
        "/api/v1/core",
        "/api/v1/analyticsx",
        "/api/v1/analytics%2Fstream",
        "/api/v1/x%2F..%2Fanalytics",
        "/api/v1/%2e%2e",
        "/api/v1/analytics;v=1",
    ] {
        let (status, body) = get_status(&router, uri).await;
        assert_ne!(
            body, "install-page",
            "{uri} is never the install's: {status}"
        );
        assert_ne!(status, StatusCode::OK, "{uri}: {body}");
    }
    let (status, body) = get_status(&router, "/api/v1/probe-link").await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "install-page"),
        "negative control: an unreserved segment is the install's"
    );
}

#[tokio::test]
async fn a_catch_all_or_deeper_parameter_under_a_reserved_prefix_is_still_refused() {
    for path in [
        "/api/v1/analytics/{*rest}",
        "/api/v1/analytics/{name}",
        "/api/public/{short_code}/{rest}",
        "/api/{*rest}",
    ] {
        let set = RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
            .route(Method::GET, path, ok, public_spec(BodyLimit::Default))
            .build();
        assert!(
            matches!(
                mounted(vec![install(set)]).await,
                Err(MountError::ReservedPrefix(_))
            ),
            "{path} is refused"
        );
    }
}

async fn length(body: Bytes) -> String {
    body.len().to_string()
}

fn sized(small: usize, large: usize) -> RouteSet {
    RouteSet::at::<ProbeState>(BODY_BASE, RouteGroup::Extensions)
        .route(
            Method::POST,
            "/small",
            length,
            public_spec(BodyLimit::Bytes(small)),
        )
        .route(
            Method::POST,
            "/large",
            length,
            public_spec(BodyLimit::Bytes(large)),
        )
        .build()
}

fn streamed(bytes: usize) -> Body {
    let chunk = Bytes::from(vec![b'x'; 64 * 1024]);
    let chunks = bytes.div_ceil(chunk.len());
    let last = bytes - (chunks - 1) * chunk.len();
    Body::from_stream(stream::iter((0..chunks).map(move |index| {
        let piece = if index + 1 == chunks {
            chunk.slice(..last)
        } else {
            chunk.clone()
        };
        Ok::<_, std::io::Error>(piece)
    })))
}

#[tokio::test]
async fn a_routes_own_byte_limit_holds_on_a_streamed_body_both_ways() {
    let mib = 1024 * 1024;
    let router = mounted(vec![install(sized(64 * 1024, 3 * mib))])
        .await
        .expect("limits within the ceiling mount");
    let post = |uri: &'static str, body: Body| send(router.clone(), Method::POST, uri, body);

    assert_eq!(
        post("/api/v1/probe-body/small", streamed(32 * 1024))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        post("/api/v1/probe-body/small", streamed(128 * 1024))
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE,
        "a streamed body past a limit smaller than the default is refused"
    );
    let (status, body) = post("/api/v1/probe-body/large", streamed(5 * mib / 2)).await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, (5 * mib / 2).to_string()),
        "a limit larger than the 2 MiB default admits a streamed body past the default"
    );
    assert_eq!(
        post("/api/v1/probe-body/large", streamed(7 * mib / 2))
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE,
        "a streamed body past the larger limit is refused"
    );
    let mut declared = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/probe-body/small")
        .header(header::CONTENT_LENGTH, "1048576")
        .body(Body::from(vec![b'x'; 1024 * 1024]))
        .expect("valid request");
    declared
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
    declared.extensions_mut().insert(anonymous());
    assert_eq!(
        router
            .clone()
            .oneshot(declared)
            .await
            .expect("infallible")
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "a declared Content-Length past the limit is refused"
    );
}

#[tokio::test]
async fn a_byte_limit_above_the_profile_ceiling_refuses_the_mount_naming_the_route() {
    match mounted(vec![install(sized(1024, CEILING + 1))]).await {
        Err(MountError::RouteBodyLimit {
            path,
            declared,
            ceiling,
            ..
        }) => {
            assert_eq!(path, "/api/v1/probe-body/large");
            assert_eq!((declared, ceiling), (CEILING + 1, CEILING));
        },
        other => panic!("expected the ceiling refusal, got {other:?}"),
    }
    assert!(
        matches!(
            mounted(vec![install(sized(0, 1024))]).await,
            Err(MountError::RouteBodyLimit { declared: 0, .. })
        ),
        "a zero limit is refused"
    );
    let _mounted = mounted(vec![install(sized(1024, CEILING))])
        .await
        .expect("negative control: a limit at the ceiling mounts");
}

async fn site(request: Request) -> String {
    let caller = request
        .extensions()
        .get::<RequestContext>()
        .map(|ctx| ctx.user_type());
    format!("site:{caller:?}")
}

async fn with_site_fallback() -> Router {
    let layers = layers().await;
    let tree = mounted(vec![reserved_owner()])
        .await
        .expect("the reserved owner mounts");
    let fallback = protect_site_fallback(
        SiteFallback::service(axum::routing::any(site).with_state(())),
        &layers,
    )
    .expect("the site fallback mounts");
    RootFallbackOwner::default()
        .place(tree, fallback)
        .expect("one fallback is placed")
}

#[tokio::test]
async fn the_site_fallback_is_public_and_never_answers_a_reserved_path() {
    let router = with_site_fallback().await;

    let (status, body) = get_status(&router, "/pricing/plans").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an unmatched path is the fallback's: {body}"
    );
    assert!(body.starts_with("site:"), "{body}");
    assert!(
        !body.contains(&format!("{:?}", UserType::User))
            && !body.contains(&format!("{:?}", UserType::Admin)),
        "the fallback is never authenticated, even with a bearer header: {body}"
    );

    let (owned, body) = get_status(&router, "/api/v1/analytics/stream").await;
    assert!(
        owned == StatusCode::UNAUTHORIZED || owned == StatusCode::FORBIDDEN,
        "a reserved route still answers under its own policy: {owned} {body}"
    );
    let (method, body) = send(
        router.clone(),
        Method::POST,
        "/api/v1/analytics/stream",
        Body::empty(),
    )
    .await;
    assert!(
        matches!(
            method,
            StatusCode::METHOD_NOT_ALLOWED | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) && !body.starts_with("site:"),
        "a method mismatch is the reserved route's own answer, not the fallback: {method} {body}"
    );
    for uri in [
        "/api/v1/core/contexts",
        "/api/v1/analytics/unknown",
        "/api/v1/%61nalytics/stream",
        "/.well-known/openid-configuration",
        "/v1/messages",
        "/livez",
        "/api/v1/analytics%2Fstream",
        "/pages/%2e%2e/api/v1/core",
        "/api/v1/core;x=1/contexts",
        "/pages/x%5Cy",
    ] {
        let (status, body) = get_status(&router, uri).await;
        assert!(
            !body.starts_with("site:"),
            "{uri} is never the fallback's: {status} {body}"
        );
    }
}

#[tokio::test]
async fn a_second_site_fallback_claim_is_refused() {
    let layers = layers().await;
    let mut owner = RootFallbackOwner::default();
    let first = protect_site_fallback(SiteFallback::service(get(ok).with_state(())), &layers)
        .expect("the first fallback protects");
    let tree = owner
        .place(Router::new(), first)
        .expect("the first claim is placed");
    let second = protect_site_fallback(SiteFallback::service(get(ok).with_state(())), &layers)
        .expect("the second fallback protects");
    assert!(matches!(
        owner.place(tree, second),
        Err(MountError::SecondRootFallback { .. })
    ));
}

#[test]
fn the_site_fallback_stays_behind_site_auth_when_a_member_declares_it() {
    let private = site_fallback_protection(true);
    assert!(
        private
            .layers
            .contains(&systemprompt_extension::ProtectionLayer::SiteAuth),
        "a private site's gate covers the fallback"
    );
    assert_eq!(private.policy, AuthzPolicy::public());
    assert_eq!(private.context, ContextKind::Public);
    assert!(
        site_fallback_protection(false).layers.is_empty(),
        "negative control: no site auth declared, no layer"
    );
}
