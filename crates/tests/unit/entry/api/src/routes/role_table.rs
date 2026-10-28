//! The per-role route table and lifecycle plan behind `server.role`.

use std::collections::HashSet;

use systemprompt_api::services::server::routes::role::{
    LifecyclePlan, ProcessOwnership, RouteGroup, lifecycle_plan, route_groups,
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
fn api_role_mounts_everything_but_the_gateway() {
    let api = set(route_groups(NodeRole::Api));
    assert!(!api.contains(&RouteGroup::Gateway));
    assert!(!api.contains(&RouteGroup::Managed));
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
        assert!(api.contains(&group), "api must mount {group:?}");
    }
}

#[test]
fn worker_role_mounts_probes_and_the_routes_api_replicas_forward_to() {
    let expected = set(&[
        RouteGroup::Agent,
        RouteGroup::McpAndStream,
        RouteGroup::Probes,
    ]);
    assert_eq!(set(route_groups(NodeRole::Worker)), expected);
}

#[test]
fn all_role_is_the_union_of_gateway_and_api() {
    let all = set(route_groups(NodeRole::All));
    let union: HashSet<_> = set(route_groups(NodeRole::Gateway))
        .union(&set(route_groups(NodeRole::Api)))
        .copied()
        .collect();
    assert_eq!(all, union);
    assert_eq!(all.len(), 12);
}

#[test]
fn only_all_and_worker_run_the_scheduler_and_supervise_processes() {
    assert_eq!(
        lifecycle_plan(NodeRole::Gateway),
        LifecyclePlan {
            reconcile_mcp: false,
            reconcile_agents: false,
            process_ownership: ProcessOwnership::None,
            scheduler: false,
            runner_services: false,
        }
    );
    assert_eq!(
        lifecycle_plan(NodeRole::Api),
        LifecyclePlan {
            reconcile_mcp: false,
            reconcile_agents: false,
            process_ownership: ProcessOwnership::None,
            scheduler: false,
            runner_services: false,
        }
    );
    assert_eq!(
        lifecycle_plan(NodeRole::Worker),
        LifecyclePlan {
            reconcile_mcp: true,
            reconcile_agents: true,
            process_ownership: ProcessOwnership::Leased,
            scheduler: true,
            runner_services: false,
        }
    );
    assert_eq!(
        lifecycle_plan(NodeRole::All),
        LifecyclePlan {
            reconcile_mcp: true,
            reconcile_agents: true,
            process_ownership: ProcessOwnership::Local,
            scheduler: true,
            runner_services: true,
        }
    );
}

#[test]
fn runner_role_serves_probes_and_runs_only_runner_services() {
    assert_eq!(route_groups(NodeRole::Runner), &[RouteGroup::Probes]);
    assert_eq!(
        lifecycle_plan(NodeRole::Runner),
        LifecyclePlan {
            reconcile_mcp: false,
            reconcile_agents: false,
            process_ownership: ProcessOwnership::None,
            scheduler: false,
            runner_services: true,
        }
    );
    let role: NodeRole = serde_json::from_str("\"runner\"").expect("runner parses");
    assert_eq!(role, NodeRole::Runner);
}

#[test]
fn role_parses_lowercase_and_defaults_to_all() {
    for role in NodeRole::ALL_ROLES {
        let json = serde_json::to_string(&role).expect("serialise");
        assert_eq!(json, format!("\"{role}\""));
        let back: NodeRole = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, role);
    }
    assert_eq!(NodeRole::default(), NodeRole::All);
    assert!(serde_json::from_str::<NodeRole>("\"Gateway\"").is_err());
}

#[test]
fn the_retired_admin_role_is_rejected_naming_its_replacements() {
    let err = serde_json::from_str::<NodeRole>("\"admin\"")
        .expect_err("admin is no longer a role")
        .to_string();
    assert!(err.contains("'api'"), "{err}");
    assert!(err.contains("'worker'"), "{err}");
}

#[test]
fn an_unknown_role_lists_the_valid_roles() {
    let err = serde_json::from_str::<NodeRole>("\"edge\"")
        .expect_err("edge is not a role")
        .to_string();
    assert!(err.contains("all, gateway, api, worker, runner"), "{err}");
}

#[test]
fn role_predicates_split_serving_from_supervision() {
    assert!(NodeRole::Api.serves_api());
    assert!(!NodeRole::Api.runs_scheduler());
    assert!(!NodeRole::Api.supervises_processes());
    assert!(!NodeRole::Api.writes_shared_caches());
    assert!(NodeRole::Api.reads_shared_services_composition());
    assert!(!NodeRole::Gateway.reads_shared_services_composition());
    assert!(!NodeRole::All.reads_shared_services_composition());
    assert!(!NodeRole::Worker.serves_api());
    assert!(NodeRole::Worker.runs_scheduler());
    assert!(NodeRole::Worker.supervises_processes());
    assert!(NodeRole::Worker.shares_process_ownership());
    assert!(NodeRole::Worker.writes_shared_caches());
    assert!(!NodeRole::All.shares_process_ownership());
    assert!(NodeRole::All.writes_shared_caches());
    assert!(!NodeRole::Gateway.supervises_processes());
    assert!(!NodeRole::Runner.runs_scheduler());
}
