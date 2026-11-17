//! The enumerating mount's wiring: it re-checks the handoff before mounting
//! anything, it mounts a set only on a role that serves the set's group, and
//! it nests an install's service behind the service def's own protection.
//! The limiter is built over the fixture database.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, ContextExtractor, McpContextMiddleware, MountError,
    MountLayers, ProtectionGuards, PublicContextMiddleware, RateLimitState,
    UserOnlyContextMiddleware, mount_admitted,
};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{BasePath, RouteSet, RouteSpec, WebBase};
use systemprompt_extension::{AuthzPolicy, ContextKind, HandleId, RouteGroup, StandingTag};
use systemprompt_models::RequestContext;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::{fixture_config, test_db_pool};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;

use super::forge::{PLAIN_LIMITS, ProbeState, analytics_set, forged, ok, routed};

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
    let config = fixture_config("postgres://localhost/admitted-mount");
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

async fn mount(owner: &str, serves: &dyn Fn(RouteGroup) -> bool) -> Result<Router, MountError> {
    let admitted = forged(
        vec![routed(
            owner,
            analytics_set(),
            vec![HandleId::AnalyticsBroadcaster],
        )],
        vec![],
    );
    mount_admitted(
        Router::new(),
        Composed {
            admitted: &admitted,
            kernel: &kernel(),
        },
        &layers().await,
        serves,
        &|router| router,
    )
}

fn kernel() -> Kernel {
    Kernel::new(
        Arc::new(StubDb),
        BackgroundTasks::new(),
        Dependencies::default(),
    )
}

async fn status(router: Router) -> StatusCode {
    status_at(router, "/api/v1/analytics/stream").await
}

async fn status_at(router: Router, uri: &str) -> StatusCode {
    let mut request = Request::get(uri)
        .body(Body::empty())
        .expect("valid request");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
    router.oneshot(request).await.expect("infallible").status()
}

#[tokio::test]
async fn mount_refuses_a_forged_owner_before_mounting() {
    assert!(matches!(
        mount("probe-forger", &|_| true).await,
        Err(MountError::PrivilegeOwnerMismatch {
            expected: CoreId::AnalyticsHttp,
            ..
        })
    ));
}

#[tokio::test]
async fn a_set_is_mounted_only_on_a_role_serving_its_group() {
    let served = mount("analytics-http", &|_| true)
        .await
        .expect("the listed set mounts");
    assert_ne!(
        status(served).await,
        StatusCode::NOT_FOUND,
        "the served set answers"
    );
    let skipped = mount("analytics-http", &|group| {
        group != RouteGroup::ContentAndMisc
    })
    .await
    .expect("a skipped set is not an error");
    assert_eq!(
        status(skipped).await,
        StatusCode::NOT_FOUND,
        "a role not serving the group mounts nothing"
    );
}

#[tokio::test]
async fn an_install_service_is_nested_behind_its_def_protection() {
    let spec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS);
    let set = RouteSet::at::<ProbeState>(BasePath::Web(WebBase::Root), RouteGroup::Extensions)
        .service("/dash", Router::new().route("/", get(ok)), spec)
        .build();
    let admitted = forged(
        vec![AdmittedRouteSet {
            standing: StandingTag::Install,
            ..routed("probe-install", set, vec![])
        }],
        vec![],
    );
    let router = mount_admitted(
        Router::new(),
        Composed {
            admitted: &admitted,
            kernel: &kernel(),
        },
        &layers().await,
        &|_| true,
        &|router| router,
    )
    .expect("an install service mounts");
    let nested = status_at(router.clone(), "/dash").await;
    assert_ne!(
        nested,
        StatusCode::NOT_FOUND,
        "the prefix reaches the mount"
    );
    assert_ne!(
        nested,
        StatusCode::OK,
        "an unauthenticated request is stopped by the def's user policy"
    );
    assert_eq!(
        status_at(router, "/elsewhere").await,
        StatusCode::NOT_FOUND,
        "negative control: nothing is mounted outside the prefix"
    );
}
