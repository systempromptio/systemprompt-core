//! The per-role route table and lifecycle plan behind `server.role`.

use std::collections::HashSet;

use systemprompt_api::services::server::routes::role::{
    LifecyclePlan, RouteGroup, lifecycle_plan, route_groups,
};
use systemprompt_manifest::profile::NodeRole;

fn set(groups: &[RouteGroup]) -> HashSet<RouteGroup> {
    groups.iter().copied().collect()
}

#[test]
fn gateway_role_mounts_only_the_gateway_surface_and_its_dependencies() {
    let expected = set(&[
        RouteGroup::Gateway,
        RouteGroup::Managed,
        RouteGroup::Oauth,
        RouteGroup::WellKnown,
        RouteGroup::Discovery,
        RouteGroup::AuthenticatedDiscovery,
    ]);
    assert_eq!(set(route_groups(NodeRole::Gateway)), expected);
}

#[test]
fn admin_role_mounts_everything_but_the_gateway() {
    let admin = set(route_groups(NodeRole::Admin));
    assert!(!admin.contains(&RouteGroup::Gateway));
    assert!(!admin.contains(&RouteGroup::Managed));
    for group in [
        RouteGroup::Agent,
        RouteGroup::McpAndStream,
        RouteGroup::ContentAndMisc,
        RouteGroup::Messaging,
        RouteGroup::Extensions,
        RouteGroup::Static,
        RouteGroup::Oauth,
        RouteGroup::Discovery,
    ] {
        assert!(admin.contains(&group), "admin must mount {group:?}");
    }
}

#[test]
fn all_role_is_the_union_of_gateway_and_admin() {
    let all = set(route_groups(NodeRole::All));
    let union: HashSet<_> = set(route_groups(NodeRole::Gateway))
        .union(&set(route_groups(NodeRole::Admin)))
        .copied()
        .collect();
    assert_eq!(all, union);
    assert_eq!(all.len(), 12);
}

#[test]
fn gateway_role_runs_no_workers() {
    assert_eq!(
        lifecycle_plan(NodeRole::Gateway),
        LifecyclePlan {
            reconcile_mcp: false,
            reconcile_agents: false,
            scheduler: false,
        }
    );
    for role in [NodeRole::All, NodeRole::Admin] {
        let plan = lifecycle_plan(role);
        assert!(plan.reconcile_mcp && plan.reconcile_agents && plan.scheduler);
    }
}

#[test]
fn role_parses_lowercase_and_defaults_to_all() {
    let role: NodeRole = serde_json::from_str("\"gateway\"").expect("gateway parses");
    assert_eq!(role, NodeRole::Gateway);
    assert_eq!(
        serde_json::to_string(&role).expect("serialise"),
        "\"gateway\""
    );
    assert_eq!(NodeRole::default(), NodeRole::All);
    assert!(serde_json::from_str::<NodeRole>("\"worker\"").is_err());
}
