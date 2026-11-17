//! A forged route set's base is judged by the kernel even when the set
//! carries no def: a reserved extension base, an extension base enclosing a
//! reserved path, and a pinned core route drifted at its base are refused.

use systemprompt_api::services::middleware::{MountError, recheck};
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES};
use systemprompt_extension::router::{ApiBase, BasePath, BaseRef, RouteSpec};
use systemprompt_extension::{AuthzPolicy, ContextKind, RouteGroup};

use super::forge::{PLAIN_LIMITS, forged, routed, set, spec_of};

fn at_extension(path: &'static str) -> Result<(), MountError> {
    let spec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS);
    recheck(&forged(
        vec![routed(
            "probe-forger",
            set(
                BasePath::Api(ApiBase::Extension(path)),
                RouteGroup::Extensions,
                &[(http::Method::GET, "/x", spec)],
            ),
            vec![],
        )],
        vec![],
    ))
}

#[test]
fn an_extension_base_held_by_or_enclosing_a_reserved_path_is_refused() {
    for (path, reserved) in [
        ("/api/v1/contentx", "/api/v1/content"),
        ("/api/v1/analyticsx", "/api/v1/analytics"),
        ("/api/v1/agentsx", "/api/v1/agents"),
        ("/api/v1/mcpx", "/api/v1/mcp"),
        ("/api/v1/admin/x", "/api/v1/admin"),
    ] {
        let result = at_extension(path);
        assert!(
            matches!(result, Err(MountError::ReservedPrefix(p)) if p == reserved),
            "{path}: {result:?}"
        );
    }
    let result = at_extension("/api");
    assert!(
        matches!(result, Err(MountError::ReservedPrefix(_))),
        "an enclosing base: {result:?}"
    );
    let result = at_extension("/api/v1/probe");
    assert!(result.is_ok(), "negative control: {result:?}");
}

#[test]
fn every_pinned_core_route_drifted_at_its_base_is_refused() {
    for listed in PRIVILEGED_SHAPES {
        let (BaseRef::Path(base), Some(routes)) = (listed.base, listed.routes) else {
            continue;
        };
        if listed.owner == CoreId::Kernel {
            continue;
        }
        for (method, path) in routes {
            let exact = spec_of(listed);
            let policy = if exact.policy == AuthzPolicy::admin() {
                AuthzPolicy::user()
            } else {
                AuthzPolicy::admin()
            };
            let context = if exact.policy == AuthzPolicy::admin() {
                exact.context
            } else {
                ContextKind::User
            };
            let drifted = RouteSpec::new(policy, context, exact.limits);
            let result = recheck(&forged(
                vec![routed(
                    listed.owner.id(),
                    set(base, listed.group, &[(method.clone(), path, drifted)]),
                    vec![],
                )],
                vec![],
            ));
            assert!(
                matches!(result, Err(MountError::UnlistedPrivilege { .. })),
                "{:?} {method} {path} drifted: {result:?}",
                listed.owner
            );
        }
    }
}
