//! Startup event type definitions.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    PreFlight,
    Database,
    McpServers,
    ApiServer,
    Agents,
    Scheduler,
}

impl Phase {
    /// Human-readable phase name for display
    pub const fn name(&self) -> &'static str {
        match self {
            Self::PreFlight => "Pre-flight",
            Self::Database => "Database",
            Self::McpServers => "MCP Servers",
            Self::ApiServer => "API Server",
            Self::Agents => "Agents",
            Self::Scheduler => "Scheduler",
        }
    }

    /// Whether this phase blocks API readiness
    pub const fn is_blocking(&self) -> bool {
        matches!(
            self,
            Self::PreFlight | Self::Database | Self::McpServers | Self::ApiServer
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    Mcp,
    Agent,
    Api,
    Scheduler,
}

impl ServiceType {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Mcp => "MCP",
            Self::Agent => "Agent",
            Self::Api => "API",
            Self::Scheduler => "Sched",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Starting,
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub service_type: ServiceType,
    pub port: Option<u16>,
    pub state: ServiceState,
    pub startup_time: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub category: String,
}
