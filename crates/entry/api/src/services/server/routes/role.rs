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

const ADMIN_GROUPS: &[RouteGroup] = &[
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

pub const fn route_groups(role: NodeRole) -> &'static [RouteGroup] {
    match role {
        NodeRole::All => ALL_GROUPS,
        NodeRole::Gateway => GATEWAY_GROUPS,
        NodeRole::Admin => ADMIN_GROUPS,
    }
}

/// Startup phases a node of a given role runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecyclePlan {
    pub reconcile_mcp: bool,
    pub reconcile_agents: bool,
    pub scheduler: bool,
}

pub const fn lifecycle_plan(role: NodeRole) -> LifecyclePlan {
    let workers = role.runs_workers();
    LifecyclePlan {
        reconcile_mcp: workers,
        reconcile_agents: workers,
        scheduler: workers,
    }
}
