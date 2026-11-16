//! The kernel re-check compares every def of a forged handoff, by its full
//! path, with the reserved paths: an install's plain def, parameter route or
//! service that reaches a kernel or core namespace is refused at mount, while
//! the same routes held by their core owner pass.

use axum::Router;
use axum::routing::get;
use systemprompt_api::services::middleware::{MountError, recheck};
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::router::{BasePath, RouteSet, RouteSpec, WebBase};
use systemprompt_extension::{AuthzPolicy, ContextKind, RouteGroup, StandingTag};

use super::forge::{PLAIN_LIMITS, ProbeState, forged, ok, routed, set};

const PLAIN: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS);
const ROOT: BasePath = BasePath::Web(WebBase::Root);

fn page(owner: &str, standing: StandingTag, path: &'static str) -> AdmittedRouteSet {
    AdmittedRouteSet {
        standing,
        ..routed(
            owner,
            set(ROOT, RouteGroup::Extensions, &[(http::Method::GET, path, PLAIN)]),
            vec![],
        )
    }
}

fn service(prefix: &'static str) -> AdmittedRouteSet {
    AdmittedRouteSet {
        standing: StandingTag::Install,
        ..routed(
            "probe-install",
            RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                .service(prefix, Router::new().route("/", get(ok)), PLAIN)
                .build(),
            vec![],
        )
    }
}

fn rechecked(set: AdmittedRouteSet) -> Result<(), MountError> {
    recheck(&forged(vec![set], vec![]))
}

#[test]
fn an_install_def_under_a_kernel_namespace_is_refused_at_mount() {
    for (path, reserved) in [
        ("/api/v1/mcp/github/mcp", "/api/v1/mcp"),
        ("/api/{version}/mcp/github/mcp", "/api/v1/mcp"),
        ("/.well-known/openid-configuration", "/.well-known"),
        ("/v1/x", "/v1"),
    ] {
        let result = rechecked(page("probe-install", StandingTag::Install, path));
        assert!(
            matches!(result, Err(MountError::ReservedPrefix(prefix)) if prefix == reserved),
            "{path}: {result:?}"
        );
    }
    for path in ["/api/v1/probe/x", "/pages/{slug}"] {
        let result = rechecked(page("probe-install", StandingTag::Install, path));
        assert!(result.is_ok(), "negative control {path}: {result:?}");
    }
}

#[test]
fn an_install_service_covering_a_kernel_namespace_is_refused_at_mount() {
    for (prefix, reserved) in [("/api", "/api/v1"), ("/api/public", "/api/public/gateway")] {
        let result = rechecked(service(prefix));
        assert!(
            matches!(result, Err(MountError::ReservedPrefix(p)) if p == reserved),
            "{prefix}: {result:?}"
        );
    }
    let result = rechecked(service("/apix"));
    assert!(result.is_ok(), "negative control: {result:?}");
}

#[test]
fn the_core_owner_holds_its_reserved_paths_at_mount() {
    for (owner, path) in [
        ("kernel", "/api/v1/mcp/github/mcp"),
        ("kernel", "/.well-known/x"),
        ("kernel", "/v1/x"),
        ("notifications-http", "/api/v1/admin/notifications/x"),
    ] {
        let result = rechecked(page(owner, StandingTag::Core, path));
        assert!(result.is_ok(), "{owner} {path}: {result:?}");
    }
    for (owner, standing, path, reserved) in [
        ("kernel", StandingTag::Core, "/api/v1/admin/notifications/x", "/api/v1/admin/notifications"),
        ("content-http", StandingTag::Core, "/api/v1/mcp/x", "/api/v1/mcp"),
        ("kernel", StandingTag::Install, "/v1/x", "/v1"),
    ] {
        let result = rechecked(page(owner, standing, path));
        assert!(
            matches!(result, Err(MountError::ReservedPrefix(p)) if p == reserved),
            "{owner} {standing:?} {path}: {result:?}"
        );
    }
}
