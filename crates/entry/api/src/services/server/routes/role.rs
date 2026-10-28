//! Per-role route table and startup lifecycle plan.
//!
//! A node's `server.role` decides which route groups it mounts and which
//! startup phases it runs. Both are pure functions of the role so the
//! per-role surface is testable without building an `AppContext`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::profile::NodeRole;

/// A group of routes mounted together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteGroup {
    Oauth,
    Agent,
    McpAndStream,
    ContentAndMisc,
    Messaging,
    Extensions,
    Discovery,
    AuthenticatedDiscovery,
    WellKnown,
    Managed,
    Gateway,
    Static,
    Probes,
}

const ALL_GROUPS: &[RouteGroup] = &[
    RouteGroup::Oauth,
    RouteGroup::Agent,
    RouteGroup::McpAndStream,
    RouteGroup::ContentAndMisc,
    RouteGroup::Messaging,
    RouteGroup::Extensions,
    RouteGroup::Discovery,
    RouteGroup::AuthenticatedDiscovery,
    RouteGroup::WellKnown,
    RouteGroup::Managed,
    RouteGroup::Gateway,
    RouteGroup::Static,
];

const GATEWAY_GROUPS: &[RouteGroup] = &[
    RouteGroup::Oauth,
    RouteGroup::Discovery,
    RouteGroup::AuthenticatedDiscovery,
    RouteGroup::WellKnown,
    RouteGroup::Managed,
    RouteGroup::Gateway,
];

const API_GROUPS: &[RouteGroup] = &[
    RouteGroup::Oauth,
    RouteGroup::Agent,
    RouteGroup::McpAndStream,
    RouteGroup::ContentAndMisc,
    RouteGroup::Messaging,
    RouteGroup::Extensions,
    RouteGroup::Discovery,
    RouteGroup::AuthenticatedDiscovery,
    RouteGroup::WellKnown,
    RouteGroup::Static,
];

const WORKER_GROUPS: &[RouteGroup] = &[
    RouteGroup::Agent,
    RouteGroup::McpAndStream,
    RouteGroup::Probes,
];

const RUNNER_GROUPS: &[RouteGroup] = &[RouteGroup::Probes];

pub const fn route_groups(role: NodeRole) -> &'static [RouteGroup] {
    match role {
        NodeRole::All => ALL_GROUPS,
        NodeRole::Gateway => GATEWAY_GROUPS,
        NodeRole::Api => API_GROUPS,
        NodeRole::Worker => WORKER_GROUPS,
        NodeRole::Runner => RUNNER_GROUPS,
    }
}

/// Startup phases a node of a given role runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecyclePlan {
    pub reconcile_mcp: bool,
    pub reconcile_agents: bool,
    pub process_ownership: ProcessOwnership,
    pub scheduler: bool,
    pub runner_services: bool,
}

/// How a node that supervises MCP and agent processes comes to own them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOwnership {
    None,
    Local,
    Leased,
}

pub const fn lifecycle_plan(role: NodeRole) -> LifecyclePlan {
    let supervises = role.supervises_processes();
    let process_ownership = if !supervises {
        ProcessOwnership::None
    } else if role.shares_process_ownership() {
        ProcessOwnership::Leased
    } else {
        ProcessOwnership::Local
    };
    LifecyclePlan {
        reconcile_mcp: supervises,
        reconcile_agents: supervises,
        process_ownership,
        scheduler: role.runs_scheduler(),
        runner_services: role.runs_runner_services(),
    }
}
