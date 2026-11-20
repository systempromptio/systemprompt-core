//! The MCP registry route set: one public `GET /` at `ApiBase::McpRegistry`
//! in the MCP-and-stream group under the `mcp_registry` rate limit, equal in
//! full to `mcp-http`'s privileged shape, which pins that one route.

use axum::http::Method;
use systemprompt_extension::reserved::{CoreId, MCP_REGISTRY_ROUTES, PRIVILEGED_SHAPES};
use systemprompt_extension::router::{ApiBase, BasePath, BaseRef, BodyLimit, RouteMethod};
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_mcp_entry::http::registry_routes;

#[test]
fn the_set_holds_exactly_the_public_registry_route() {
    let set = registry_routes();
    assert_eq!(
        set.base(),
        BaseRef::Path(BasePath::Api(ApiBase::McpRegistry))
    );
    assert_eq!(set.group(), RouteGroup::McpAndStream);
    let defs = set.defs();
    assert_eq!(defs.len(), 1, "{defs:?}");
    let def = &defs[0];
    assert_eq!(def.method, RouteMethod::One(Method::GET));
    assert_eq!(def.path.as_str(), "/");
    assert_eq!(def.spec.policy, AuthzPolicy::public());
    assert_eq!(def.spec.context, ContextKind::Public);
    assert_eq!(
        def.spec.limits.rate,
        RateLimit::new("mcp_registry", RateBudget::McpRegistry)
    );
    assert_eq!(def.spec.limits.body, BodyLimit::Default);
}

#[test]
fn the_set_equals_the_pinned_privileged_shape() {
    let set = registry_routes();
    let entries: Vec<_> = PRIVILEGED_SHAPES
        .iter()
        .filter(|entry| entry.owner == CoreId::McpHttp)
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let entry = entries[0];
    assert_eq!(entry.base, set.base());
    assert_eq!(entry.group, set.group());
    assert_eq!(entry.routes, Some(MCP_REGISTRY_ROUTES));
    assert_eq!(MCP_REGISTRY_ROUTES, &[(Method::GET, "/")]);
    for def in set.defs() {
        assert_eq!(entry.shape, def.spec.shape());
    }
}
