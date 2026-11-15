//! A handoff built through the hidden facade constructor gains nothing
//! beyond a shape the kernel constants list, under the core id they list it
//! for (T2 and T16 kernel half, T9, T14, T15, T17 mount half, T18 kernel half).

use std::sync::Arc;

use axum::Router;
use axum::http::HeaderMap;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, ContextExtractor, McpContextMiddleware, MountError,
    MountLayers, ProtectionGuards, PublicContextMiddleware, RateLimitState,
    UserOnlyContextMiddleware, mount_admitted,
};
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedParts, AdmittedRouteSet};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayPath, GatewayRouteDecl, RouteLimits,
    RouteSet, RouteSpec,
};
use systemprompt_extension::{
    Admitted, AuthzPolicy, ContextKind, FromState, HandleId, HandlerVerifier, MissingDependency,
    RateBudget, RateLimit, Resolved, RouteGroup, StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::RequestContext;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::{fixture_config, test_db_pool};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};
use systemprompt_users::UserRateLimitBucketRepository;

#[derive(Clone)]
struct NoContext;

#[async_trait::async_trait]
impl ContextExtractor for NoContext {
    async fn extract_from_headers(&self, _headers: &HeaderMap) -> Result<RequestContext, ContextExtractionError> {
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

#[derive(Clone)]
struct ProbeState;

impl FromState for ProbeState {
    fn from_state(_r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self)
    }
}

async fn ok() -> &'static str {
    "ok"
}

async fn layers() -> MountLayers {
    let config = fixture_config("postgres://localhost/forged");
    let db = test_db_pool().await;
    MountLayers {
        limits: RateLimitState::new(&config, Arc::new(UserRateLimitBucketRepository::new(&db))),
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(NoContext),
        a2a: A2AContextMiddleware::new(NoContext, 1024, BodyBudget::new(64 * 1024 * 1024)),
        mcp: McpContextMiddleware::new(NoContext),
        bearer: None,
        guards: ProtectionGuards::default(),
    }
}

async fn mount(admitted: &Admitted) -> Result<Router, MountError> {
    let kernel = Kernel::new(Arc::new(StubDb), BackgroundTasks::new(), Dependencies::default());
    mount_admitted(Router::new(), admitted, &kernel, &layers().await, &|_| true, &|router| router)
}

const PLAIN_LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("probe", RateBudget::Contexts),
    body: BodyLimit::Default,
};

fn entry(owner: CoreId, base: BaseRef, allowance: Allowance) -> &'static PrivilegedShape {
    PRIVILEGED_SHAPES
        .iter()
        .find(|e| e.owner == owner && e.base == base && e.allowance == allowance)
        .expect("the privileged shape is listed")
}

const fn spec_of(entry: &PrivilegedShape) -> RouteSpec {
    RouteSpec::new(entry.shape.policy, entry.shape.context, entry.shape.limits)
}

fn set(base: BasePath, group: RouteGroup, routes: &[(http::Method, &'static str, RouteSpec)]) -> RouteSet {
    routes
        .iter()
        .fold(RouteSet::at::<ProbeState>(base, group), |builder, (method, path, spec)| {
            builder.route(method.clone(), path, ok, *spec)
        })
        .build()
}

fn routed(owner: &str, set: RouteSet, handles: Vec<HandleId>) -> AdmittedRouteSet {
    AdmittedRouteSet {
        owner: ExtensionId::new(owner),
        standing: StandingTag::Core,
        set,
        handles,
    }
}

fn forged(routes: Vec<AdmittedRouteSet>, gateway_routes: Vec<AdmittedGatewayRoute>) -> Admitted {
    Admitted::__from_facade(AdmittedParts {
        members: Vec::new(),
        routes,
        gateway_routes,
    })
}

const ANALYTICS: BasePath = BasePath::Api(ApiBase::Analytics);

fn analytics_set() -> RouteSet {
    let shape = entry(CoreId::AnalyticsHttp, BaseRef::Path(ANALYTICS), Allowance::ReservedPrefix);
    set(ANALYTICS, shape.group, &[(http::Method::GET, "/stream", spec_of(shape))])
}

fn is_recheck_refusal(result: &Result<Router, MountError>) -> bool {
    matches!(
        result,
        Err(MountError::UnlistedPrivilege { .. }
            | MountError::PrivilegeOwnerMismatch { .. }
            | MountError::ReservedPrefix(_)
            | MountError::GatewayShadowsBuiltin(_)
            | MountError::RouteConflict { .. })
    )
}

#[tokio::test]
async fn handler_verified_at_an_unlisted_base_is_refused() {
    let base = BasePath::Api(ApiBase::Extension("/api/probe"));
    let spec = RouteSpec::new(
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
        PLAIN_LIMITS,
    );
    let admitted = forged(
        vec![routed("probe-forger", set(base, RouteGroup::Messaging, &[(http::Method::POST, "/events", spec)]), vec![])],
        vec![],
    );
    assert!(matches!(
        mount(&admitted).await,
        Err(MountError::UnlistedPrivilege {
            allowance: Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            ..
        })
    ));
}

#[tokio::test]
async fn listed_privilege_under_a_non_core_id_is_an_owner_mismatch() {
    let admitted = forged(
        vec![routed("probe-forger", analytics_set(), vec![HandleId::AnalyticsBroadcaster])],
        vec![],
    );
    assert!(matches!(
        mount(&admitted).await,
        Err(MountError::PrivilegeOwnerMismatch {
            expected: CoreId::AnalyticsHttp,
            ..
        })
    ));
}

#[tokio::test]
async fn one_id_holding_two_core_ids_is_an_owner_mismatch() {
    let content = entry(CoreId::ContentHttp, BaseRef::Path(BasePath::Api(ApiBase::Content)), Allowance::ReservedPrefix);
    let content_set = set(
        BasePath::Api(ApiBase::Content),
        content.group,
        &[(http::Method::GET, "/probe", spec_of(content))],
    );
    let admitted = forged(
        vec![
            routed("content-http", content_set, vec![]),
            routed("content-http", analytics_set(), vec![]),
        ],
        vec![],
    );
    assert!(matches!(
        mount(&admitted).await,
        Err(MountError::PrivilegeOwnerMismatch {
            expected: CoreId::AnalyticsHttp,
            ..
        })
    ));
}

#[tokio::test]
async fn listed_privilege_under_its_core_id_passes_the_recheck() {
    let admitted = forged(
        vec![routed("analytics-http", analytics_set(), vec![HandleId::AnalyticsBroadcaster])],
        vec![],
    );
    assert!(!is_recheck_refusal(&mount(&admitted).await), "negative control");
}

#[tokio::test]
async fn slack_shape_is_listed_only_in_a_slack_build() {
    let base = BasePath::Api(ApiBase::Slack);
    let spec = RouteSpec::new(
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
        RouteLimits {
            rate: RateLimit::new("agents", RateBudget::Agents),
            body: BodyLimit::Default,
        },
    );
    let admitted = forged(
        vec![routed("slack", set(base, RouteGroup::Messaging, &[(http::Method::POST, "/events", spec)]), vec![])],
        vec![],
    );
    let listed = PRIVILEGED_SHAPES.iter().any(|e| e.owner == CoreId::Slack);
    let result = mount(&admitted).await;
    if listed {
        assert!(!is_recheck_refusal(&result), "a slack build lists the slack shape");
    } else {
        assert!(matches!(
            result,
            Err(MountError::UnlistedPrivilege {
                allowance: Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
                ..
            })
        ));
    }
}

#[tokio::test]
async fn every_listed_shape_refuses_a_changed_policy() {
    for listed in PRIVILEGED_SHAPES {
        let BaseRef::Path(base) = listed.base else {
            continue;
        };
        if matches!(listed.allowance, Allowance::KernelHandle(_)) {
            continue;
        }
        let policy = if listed.shape.policy == AuthzPolicy::public() {
            AuthzPolicy::admin()
        } else {
            AuthzPolicy::public()
        };
        let context = if policy == AuthzPolicy::public() { ContextKind::Public } else { ContextKind::User };
        let spec = RouteSpec::new(policy, context, listed.shape.limits);
        let (method, path) = listed
            .routes
            .and_then(|routes| routes.first().cloned())
            .unwrap_or((http::Method::GET, "/probe"));
        let admitted = forged(
            vec![routed(listed.owner.id(), set(base, listed.group, &[(method, path, spec)]), vec![])],
            vec![],
        );
        let result = mount(&admitted).await;
        assert!(
            matches!(result, Err(MountError::UnlistedPrivilege { .. } | MountError::ReservedPrefix(_))),
            "{:?} at {base:?}: {:?}",
            listed.owner,
            result.err()
        );
    }
}

#[tokio::test]
async fn content_generate_with_the_public_shape_or_a_changed_group_is_refused() {
    let content = BasePath::Api(ApiBase::Content);
    let public = entry(CoreId::ContentHttp, BaseRef::Path(content), Allowance::ReservedPrefix);
    let admitted = forged(
        vec![routed(
            "content-http",
            set(content, public.group, &[(http::Method::POST, "/links/generate", spec_of(public))]),
            vec![],
        )],
        vec![],
    );
    assert!(matches!(mount(&admitted).await, Err(MountError::UnlistedPrivilege { .. })));

    let admitted = forged(
        vec![routed(
            "content-http",
            set(content, RouteGroup::Messaging, &[(http::Method::GET, "/probe", spec_of(public))]),
            vec![],
        )],
        vec![],
    );
    assert!(matches!(mount(&admitted).await, Err(MountError::UnlistedPrivilege { .. })));
}

#[tokio::test]
async fn reserved_extension_base_is_refused_at_mount() {
    let base = BasePath::Api(ApiBase::Extension("/api/v1/contentx"));
    let admitted = forged(
        vec![routed(
            "probe-forger",
            set(base, RouteGroup::Extensions, &[(http::Method::GET, "/x", RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS))]),
            vec![],
        )],
        vec![],
    );
    assert!(matches!(mount(&admitted).await, Err(MountError::ReservedPrefix(_))));
}

fn bridge_route(limits: RouteLimits) -> AdmittedGatewayRoute {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    let spec = RouteSpec::new(shape.shape.policy, shape.shape.context, limits);
    AdmittedGatewayRoute {
        owner: ExtensionId::new("notifications-http"),
        decl: GatewayRouteDecl {
            path: GatewayPath::BridgeNotifications,
            limits,
            router: set(
                BasePath::Api(ApiBase::Extension("/api/bridge-probe")),
                shape.group,
                &[(http::Method::GET, "/", spec)],
            ),
        },
    }
}

#[tokio::test]
async fn bridge_gateway_route_needs_the_bearer_window() {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    assert!(shape.shape.limits.rate.bearer_window, "the listed shape carries the bearer window");
    let mut without = shape.shape.limits;
    without.rate = RateLimit::new(without.rate.scope, without.rate.budget);
    assert!(matches!(
        mount(&forged(vec![], vec![bridge_route(without)])).await,
        Err(MountError::UnlistedPrivilege {
            allowance: Allowance::GatewayRoute,
            ..
        })
    ));
    assert!(
        !is_recheck_refusal(&mount(&forged(vec![], vec![bridge_route(shape.shape.limits)])).await),
        "negative control: the listed limits pass the recheck"
    );
}
