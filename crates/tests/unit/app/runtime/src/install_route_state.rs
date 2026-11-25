//! Route state built from member state at materialise: a route set's
//! late-built service and a site fallback's service are built from the
//! member's own route state `S`, resolved through the member-scoped route
//! resolution, so they read the member's provided values and never a
//! kernel or host-sealed seed (Rule C).

use std::sync::{Arc, Mutex};

use axum::routing::any;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec, SiteFallback,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, FromState, ProvideError, RateBudget, RateLimit, Resolved, RouteGroup,
    StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::{Config, ConfigView};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::BackgroundTasks;

const OWNER: &str = "plane-probe";
const BASE: BasePath = BasePath::Api(ApiBase::Extension("/api/plane-probe"));
const SPEC: RouteSpec = RouteSpec::new(
    AuthzPolicy::user(),
    ContextKind::User,
    RouteLimits {
        rate: RateLimit::new("plane-probe", RateBudget::Contexts),
        body: BodyLimit::Default,
    },
);

#[derive(Debug)]
struct Plane(&'static str);

#[derive(Clone)]
struct PlaneState(Arc<Plane>);

impl FromState for PlaneState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self(Arc::clone(&r.get::<Arc<Plane>>()?)))
    }
}

#[derive(Clone)]
struct SealedState;

impl FromState for SealedState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        r.get::<Arc<Config>>()?;
        Ok(Self)
    }
}

#[derive(Clone)]
struct ViewState;

impl FromState for ViewState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        r.get::<Arc<ConfigView>>()?;
        Ok(Self)
    }
}

async fn kernel() -> Kernel {
    ensure_test_bootstrap();
    let ctx: Arc<AppContext> =
        systemprompt_test_fixtures::test_app_context(&test_database_url()).await;
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel
        .resolve(&probe_admitted(ComposedParts {
            probes: vec![Probe::new(OWNER, |c| {
                c.provide(|_: &Resolved<'_>| -> Result<Arc<Plane>, ProvideError> {
                    Ok(Arc::new(Plane("member plane")))
                });
            })],
            ..ComposedParts::default()
        }))
        .expect("the probe resolves");
    kernel
}

#[tokio::test]
async fn a_late_built_service_is_built_from_the_members_own_state() {
    let kernel = kernel().await;
    let owner = ExtensionId::new(OWNER);
    let seen: Arc<Mutex<Option<&'static str>>> = Arc::default();
    let record = Arc::clone(&seen);
    let set = RouteSet::at::<PlaneState>(BASE, RouteGroup::Extensions)
        .service_with(
            "/plane",
            move |state: &PlaneState| {
                *record.lock().expect("lock") = Some(state.0.0);
                any(|| async { "plane" }).with_state(())
            },
            SPEC,
        )
        .build();
    assert_eq!(set.defs().len(), 1);
    let routers = set
        .materialize(&kernel.route_scoped(&owner, StandingTag::Install, &[]))
        .expect("the member's own provided value resolves");
    assert_eq!(routers.len(), 1);
    assert_eq!(*seen.lock().expect("lock"), Some("member plane"));

    let fallback = SiteFallback::with_state(|state: PlaneState| {
        let body = state.0.0;
        any(move || async move { body }).with_state(())
    });
    let _router = fallback
        .materialize(&kernel.route_scoped(&owner, StandingTag::Install, &[]))
        .expect("a fallback built from member state resolves");
}

#[tokio::test]
async fn a_late_built_service_cannot_name_a_host_seed() {
    let kernel = kernel().await;
    let owner = ExtensionId::new(OWNER);
    for standing in [StandingTag::Install, StandingTag::Core] {
        let route = kernel.route_scoped(&owner, standing, &[]);
        let sealed = RouteSet::at::<SealedState>(BASE, RouteGroup::Extensions)
            .service_with(
                "/plane",
                |_: &SealedState| any(|| async { "x" }).with_state(()),
                SPEC,
            )
            .build();
        let missing = sealed
            .materialize(&route)
            .expect_err("route state never reads the sealed Config");
        assert_eq!(missing.type_name(), std::any::type_name::<Arc<Config>>());
        let view = RouteSet::at::<ViewState>(BASE, RouteGroup::Extensions)
            .service_with(
                "/plane",
                |_: &ViewState| any(|| async { "x" }).with_state(()),
                SPEC,
            )
            .build();
        assert!(
            view.materialize(&route).is_err(),
            "an install seed is a provide-fn read, never route state"
        );
        let fallback =
            SiteFallback::with_state(|_: SealedState| any(|| async { "x" }).with_state(()));
        assert!(fallback.materialize(&route).is_err());
    }
}
