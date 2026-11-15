//! Forged admission handoffs: route sets and gateway routes stamped with any
//! owner, assembled through the hidden facade constructor.

use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedParts, AdmittedRouteSet};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES, PrivilegedShape};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{
    Admitted, FromState, HandleId, MissingDependency, RateBudget, RateLimit, Resolved, RouteGroup,
    StandingTag,
};
use systemprompt_identifiers::ExtensionId;

#[derive(Clone)]
pub(super) struct ProbeState;

impl FromState for ProbeState {
    fn from_state(_r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self)
    }
}

pub(super) async fn ok() -> &'static str {
    "ok"
}

pub(super) const PLAIN_LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("probe", RateBudget::Contexts),
    body: BodyLimit::Default,
};

pub(super) fn entry(owner: CoreId, base: BaseRef, allowance: Allowance) -> &'static PrivilegedShape {
    PRIVILEGED_SHAPES
        .iter()
        .find(|e| e.owner == owner && e.base == base && e.allowance == allowance)
        .expect("the privileged shape is listed")
}

pub(super) const fn spec_of(entry: &PrivilegedShape) -> RouteSpec {
    RouteSpec::new(entry.shape.policy, entry.shape.context, entry.shape.limits)
}

pub(super) fn set(base: BasePath, group: RouteGroup, routes: &[(http::Method, &'static str, RouteSpec)]) -> RouteSet {
    routes
        .iter()
        .fold(RouteSet::at::<ProbeState>(base, group), |builder, (method, path, spec)| {
            builder.route(method.clone(), path, ok, *spec)
        })
        .build()
}

pub(super) fn routed(owner: &str, set: RouteSet, handles: Vec<HandleId>) -> AdmittedRouteSet {
    AdmittedRouteSet {
        owner: ExtensionId::new(owner),
        standing: StandingTag::Core,
        set,
        handles,
    }
}

pub(super) fn forged(routes: Vec<AdmittedRouteSet>, gateway_routes: Vec<AdmittedGatewayRoute>) -> Admitted {
    Admitted::__from_facade(AdmittedParts {
        members: Vec::new(),
        routes,
        gateway_routes,
    })
}

pub(super) const ANALYTICS: BasePath = BasePath::Api(ApiBase::Analytics);

pub(super) fn analytics_set() -> RouteSet {
    let shape = entry(CoreId::AnalyticsHttp, BaseRef::Path(ANALYTICS), Allowance::ReservedPrefix);
    set(ANALYTICS, shape.group, &[(http::Method::GET, "/stream", spec_of(shape))])
}
