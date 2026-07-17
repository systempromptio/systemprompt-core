//! Pure lifecycle planning for service start and restart commands.
//!
//! [`StartupPlan`] and [`RestartPlan`] turn a requested scope plus the
//! observed service state into the exact set of actions a caller performs.
//! Both computations are I/O-free, so composition roots (the CLI) can build
//! the inputs, compute the plan, render progress, and drive orchestrators —
//! while the decision logic stays here and stays table-testable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::ServiceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupRequest {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
    pub skip_migrate: bool,
}

/// Agents and MCP servers are managed by the API server lifecycle, so a
/// standalone agents/MCP request yields an advisory notice, not a start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupPlan {
    pub run_migrations: bool,
    pub start_api: bool,
    pub agents_standalone_notice: bool,
    pub mcp_standalone_notice: bool,
}

impl StartupPlan {
    #[must_use]
    pub const fn compute(request: StartupRequest) -> Self {
        Self {
            run_migrations: !request.skip_migrate,
            start_api: request.api,
            agents_standalone_notice: request.agents && !request.api,
            mcp_standalone_notice: request.mcp && !request.api,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartScope {
    AllAgents,
    AllMcp,
    Failed,
}

/// `id` is the registry identifier used to drive the orchestrator; `name` is
/// the display name. They differ for agents (registry id vs configured name)
/// and coincide for MCP servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSnapshot {
    pub service_type: ServiceType,
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestartTarget {
    pub service_type: ServiceType,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RestartPlan {
    pub targets: Vec<RestartTarget>,
}

impl RestartPlan {
    #[must_use]
    pub fn compute(scope: RestartScope, snapshot: &[ServiceSnapshot]) -> Self {
        let targets = snapshot
            .iter()
            .filter(|service| service.enabled)
            .filter(|service| match scope {
                RestartScope::AllAgents => service.service_type == ServiceType::Agent,
                RestartScope::AllMcp => service.service_type == ServiceType::Mcp,
                RestartScope::Failed => !service.healthy,
            })
            .map(|service| RestartTarget {
                service_type: service.service_type,
                id: service.id.clone(),
                name: service.name.clone(),
            })
            .collect();
        Self { targets }
    }
}
