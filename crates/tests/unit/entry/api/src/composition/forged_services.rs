//! The kernel re-check of a nested service in a forged handoff: held only by
//! an install, at an unreserved extension base or the web root with no other
//! allowance, under no reserved prefix, and covering no other route.

use axum::Router;
use axum::routing::get;
use systemprompt_api::services::middleware::{MountError, recheck};
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::router::{ApiBase, BasePath, RouteMethod, RouteSet, RouteSpec, WebBase};
use systemprompt_extension::{AuthzPolicy, ContextKind, RouteGroup, StandingTag};

use super::forge::{PLAIN_LIMITS, ProbeState, forged, ok, routed, set};

const PLAIN: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS);
const ROOT: BasePath = BasePath::Web(WebBase::Root);
const EXT: BasePath = BasePath::Api(ApiBase::Extension("/api/probe"));

fn service_set(base: BasePath, prefix: &'static str) -> RouteSet {
    RouteSet::at::<ProbeState>(base, RouteGroup::Extensions)
        .service(prefix, Router::new().route("/", get(ok)), PLAIN)
        .build()
}

fn held(standing: StandingTag, set: RouteSet) -> AdmittedRouteSet {
    AdmittedRouteSet {
        standing,
        ..routed("probe-install", set, vec![])
    }
}

#[test]
fn a_service_held_by_a_core_member_is_refused() {
    for base in [ROOT, EXT] {
        let admitted = forged(vec![held(StandingTag::Core, service_set(base, "/dash"))], vec![]);
        assert!(
            matches!(
                recheck(&admitted),
                Err(MountError::ServiceNotAdmitted { base: at, .. }) if at == base
            ),
            "{base:?}"
        );
        let admitted = forged(
            vec![held(StandingTag::Install, service_set(base, "/dash"))],
            vec![],
        );
        assert!(
            recheck(&admitted).is_ok(),
            "negative control: an install's service passes at {base:?}"
        );
    }
}

#[test]
fn a_service_at_a_closed_core_base_is_refused() {
    let base = BasePath::Api(ApiBase::Notifications);
    let admitted = forged(
        vec![held(StandingTag::Install, service_set(base, "/dash"))],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::ServiceNotAdmitted { .. })
    ));
}

#[test]
fn a_service_under_a_reserved_prefix_is_refused() {
    let admitted = forged(
        vec![held(StandingTag::Install, service_set(ROOT, "/.well-known"))],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::ReservedPrefix("/.well-known"))
    ));
    let admitted = forged(
        vec![held(StandingTag::Install, service_set(ROOT, "/well-known"))],
        vec![],
    );
    assert!(recheck(&admitted).is_ok(), "negative control");
}

#[test]
fn a_service_covering_another_route_conflicts() {
    let page = held(
        StandingTag::Install,
        set(
            ROOT,
            RouteGroup::Extensions,
            &[(http::Method::GET, "/dash/home", PLAIN)],
        ),
    );
    let admitted = forged(
        vec![page, held(StandingTag::Install, service_set(ROOT, "/dash"))],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::RouteConflict { method: RouteMethod::Any, path }) if path == "/dash"
    ));

    let sibling = held(
        StandingTag::Install,
        set(
            ROOT,
            RouteGroup::Extensions,
            &[(http::Method::GET, "/dashboard", PLAIN)],
        ),
    );
    let admitted = forged(
        vec![sibling, held(StandingTag::Install, service_set(ROOT, "/dash"))],
        vec![],
    );
    assert!(recheck(&admitted).is_ok(), "negative control");
}
