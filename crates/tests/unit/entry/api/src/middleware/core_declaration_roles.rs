//! Role exposure through `extension_served`: a listed core declaration is
//! served by every role that serves its own route group, and every other
//! extension declaration only where the role serves both
//! `RouteGroup::Extensions` and its own group.

use axum::Router;
use systemprompt_api::services::middleware::{CoreDeclaration, extension_served};
use systemprompt_api::services::server::routes::role::route_groups;
use systemprompt_extension::{AuthzPolicy, ContextKind, ExtensionRouter, RouteGroup};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_models::modules::ApiPaths;

use super::core_declaration::row;

const ROLES: [NodeRole; 5] = [
    NodeRole::All,
    NodeRole::Gateway,
    NodeRole::Api,
    NodeRole::Worker,
    NodeRole::Runner,
];

fn served_on(role: NodeRole, extension: &'static str, declared: &ExtensionRouter) -> bool {
    let groups = route_groups(role);
    extension_served(extension, declared, &|group| groups.contains(&group))
}

#[test]
fn an_unlisted_agent_group_declaration_keeps_its_base_role_exposure() {
    let declared = row(CoreDeclaration::AgentRuns);
    for role in ROLES {
        let groups = route_groups(role);
        let base = groups.contains(&RouteGroup::Extensions) && groups.contains(&RouteGroup::Agent);
        assert_eq!(
            served_on(role, "install", &declared),
            base,
            "{role:?} must serve an install extension's agent-group router exactly as before"
        );
    }
    assert!(!served_on(NodeRole::Worker, "install", &declared));
}

#[test]
fn the_listed_agent_runs_declaration_is_served_on_workers() {
    let declared = row(CoreDeclaration::AgentRuns);
    let serving: Vec<_> = ROLES
        .into_iter()
        .filter(|role| served_on(*role, "agent_runs_api", &declared))
        .collect();
    assert_eq!(
        serving,
        vec![NodeRole::All, NodeRole::Api, NodeRole::Worker]
    );
}

#[test]
fn a_listed_id_at_another_path_or_group_gets_no_core_exposure() {
    let other_path = ExtensionRouter::new(
        Router::new(),
        "/api/v1/agent-runs-extra",
        AuthzPolicy::user(),
        ContextKind::User,
    )
    .in_group(RouteGroup::Agent)
    .with_rate_limit(CoreDeclaration::AgentRuns.rate_limit());
    assert!(!served_on(NodeRole::Worker, "agent_runs_api", &other_path));

    let other_group = row(CoreDeclaration::AgentRuns).in_group(RouteGroup::McpAndStream);
    assert!(!served_on(NodeRole::Worker, "agent_runs_api", &other_group));

    assert_eq!(CoreDeclaration::AgentRuns.base_path(), ApiPaths::AGENT_RUNS);
}

#[test]
fn an_install_extensions_default_group_is_served_by_all_and_api_only() {
    let declared = ExtensionRouter::new(
        Router::new(),
        "/api/v1/install",
        AuthzPolicy::user(),
        ContextKind::User,
    );
    let serving: Vec<_> = ROLES
        .into_iter()
        .filter(|role| served_on(*role, "install", &declared))
        .collect();
    assert_eq!(serving, vec![NodeRole::All, NodeRole::Api]);
}
