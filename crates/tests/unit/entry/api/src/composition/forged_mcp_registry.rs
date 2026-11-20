//! `mcp-http` holds exactly `/api/v1/mcp/registry` inside the kernel's
//! `/api/v1/mcp` namespace, where the MCP proxy receives callers' bearer
//! tokens. At the mount re-check its conferred set passes, and any other path
//! under `/api/v1/mcp` it declares is refused: a second route at its base, a
//! literal sibling or child of the registry path, a parameter or catch-all
//! reaching the proxy's routes, and a service prefix.

use axum::Router;
use axum::routing::get;
use http::Method;
use systemprompt_api::services::middleware::{MountError, recheck};
use systemprompt_extension::admitted::AdmittedRouteSet;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{Allowance, ApiBase, BasePath, BaseRef, RouteSet, WebBase};
use systemprompt_extension::{RouteGroup, StandingTag};

use super::forge::{PLAIN_LIMITS, ProbeState, entry, forged, ok, routed, set, spec_of};

const REGISTRY: BasePath = BasePath::Api(ApiBase::McpRegistry);
const ROOT: BasePath = BasePath::Web(WebBase::Root);
const OWNER: &str = CoreId::McpHttp.id();

fn rechecked(set: AdmittedRouteSet) -> Result<(), MountError> {
    recheck(&forged(vec![set], vec![]))
}

fn registry_set(paths: &[&'static str]) -> AdmittedRouteSet {
    let shape = entry(
        CoreId::McpHttp,
        BaseRef::Path(REGISTRY),
        Allowance::ReservedPrefix,
    );
    let routes: Vec<_> = paths
        .iter()
        .map(|path| (Method::GET, *path, spec_of(shape)))
        .collect();
    routed(OWNER, set(REGISTRY, shape.group, &routes), vec![])
}

#[test]
fn the_conferred_registry_set_passes() {
    let result = rechecked(registry_set(&["/"]));
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_second_route_at_the_registry_base_is_refused() {
    for path in ["/x", "/{name}", "/{*rest}"] {
        let result = rechecked(registry_set(&["/", path]));
        assert!(result.is_err(), "{path}: {result:?}");
    }
}

#[test]
fn mcp_http_reaching_any_other_path_under_the_mcp_base_is_refused() {
    let spec = spec_of(entry(
        CoreId::McpHttp,
        BaseRef::Path(REGISTRY),
        Allowance::ReservedPrefix,
    ));
    for path in [
        "/api/v1/mcp/registryx",
        "/api/v1/mcp/registry/x",
        "/api/v1/mcp/github/mcp",
        "/api/v1/mcp/{service_name}/mcp",
        "/api/v1/mcp/{*rest}",
        "/api/v1/mcp",
    ] {
        let forged_set = routed(
            OWNER,
            set(ROOT, RouteGroup::McpAndStream, &[(Method::GET, path, spec)]),
            vec![],
        );
        let result = rechecked(forged_set);
        assert!(
            matches!(result, Err(MountError::ReservedPrefix("/api/v1/mcp"))),
            "{path}: {result:?}"
        );
    }
}

#[test]
fn a_service_prefix_under_the_mcp_base_is_refused() {
    for (standing, owner) in [
        (StandingTag::Core, OWNER),
        (StandingTag::Install, "probe-install"),
    ] {
        let set = AdmittedRouteSet {
            standing,
            ..routed(
                owner,
                RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                    .service(
                        "/api/v1/mcp/registry",
                        Router::new().route("/", get(ok)),
                        systemprompt_extension::router::RouteSpec::new(
                            systemprompt_extension::AuthzPolicy::user(),
                            systemprompt_extension::ContextKind::User,
                            PLAIN_LIMITS,
                        ),
                    )
                    .build(),
                vec![],
            )
        };
        let result = rechecked(set);
        assert!(result.is_err(), "{owner}: {result:?}");
    }
}

#[test]
fn another_core_member_cannot_hold_the_registry_path() {
    let shape = entry(
        CoreId::McpHttp,
        BaseRef::Path(REGISTRY),
        Allowance::ReservedPrefix,
    );
    let result = rechecked(routed(
        CoreId::ContentHttp.id(),
        set(REGISTRY, shape.group, &[(Method::GET, "/", spec_of(shape))]),
        vec![],
    ));
    assert!(result.is_err(), "{result:?}");
}
