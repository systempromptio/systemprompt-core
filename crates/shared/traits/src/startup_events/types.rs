//! Startup event type definitions.

use std::time::Duration;

/// Coarse-grained phase identifier used to group startup events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Pre-flight checks (paths, permissions, ports).
    PreFlight,
    /// Database connection and migration.
    Database,
    /// MCP server bring-up.
    McpServers,
    /// HTTP API server bring-up.
    ApiServer,
    /// Agent bring-up.
    Agents,
    /// Scheduler bring-up.
    Scheduler,
}

impl Phase {
    /// Return the human-readable label for this phase.
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

    /// Whether failure in this phase aborts startup.
    pub const fn is_blocking(&self) -> bool {
        matches!(
            self,
            Self::PreFlight | Self::Database | Self::McpServers | Self::ApiServer
        )
    }
}

/// Service category used by [`ServiceInfo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    /// MCP server.
    Mcp,
    /// Agent runtime.
    Agent,
    /// HTTP API server.
    Api,
    /// Scheduler.
    Scheduler,
}

impl ServiceType {
    /// Short label suitable for log lines and dashboards.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Mcp => "MCP",
            Self::Agent => "Agent",
            Self::Api => "API",
            Self::Scheduler => "Sched",
        }
    }
}

/// Lifecycle state of a service reported during startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Currently starting up.
    Starting,
    /// Started successfully.
    Running,
    /// Stopped intentionally.
    Stopped,
    /// Failed during startup or run.
    Failed,
}

/// Snapshot of a service that has progressed through startup.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// Service name.
    pub name: String,
    /// Service category.
    pub service_type: ServiceType,
    /// Bound port, if applicable.
    pub port: Option<u16>,
    /// Current lifecycle state.
    pub state: ServiceState,
    /// Startup duration if completed.
    pub startup_time: Option<Duration>,
}

/// Description of a single registered module emitted at startup.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Module name.
    pub name: String,
    /// Coarse classification (`infra`, `domain`, `app`, ...).
    pub category: String,
}
