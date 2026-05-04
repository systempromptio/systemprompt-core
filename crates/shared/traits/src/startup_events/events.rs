//! Startup event variants emitted by the runtime as it brings services up.

use std::time::Duration;

use super::{ModuleInfo, Phase, ServiceInfo};

/// Event emitted at every notable point of the startup sequence.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StartupEvent {
    /// A startup [`Phase`] began.
    PhaseStarted {
        /// Which phase started.
        phase: Phase,
    },
    /// A startup [`Phase`] completed successfully.
    PhaseCompleted {
        /// Which phase completed.
        phase: Phase,
    },
    /// A startup [`Phase`] failed.
    PhaseFailed {
        /// Which phase failed.
        phase: Phase,
        /// Failure description.
        error: String,
    },

    /// Pre-flight check started looking at a port.
    PortCheckStarted {
        /// Port being inspected.
        port: u16,
    },
    /// A port was found to be available.
    PortAvailable {
        /// Port that was free.
        port: u16,
    },
    /// A port was found to be already bound.
    PortConflict {
        /// Port already in use.
        port: u16,
        /// PID currently bound to the port.
        pid: u32,
    },
    /// A previously conflicting port was freed.
    PortConflictResolved {
        /// Port that became available.
        port: u16,
    },
    /// All registered modules finished loading.
    ModulesLoaded {
        /// Total module count.
        count: usize,
        /// Per-module summaries.
        modules: Vec<ModuleInfo>,
    },

    /// Database migration phase began.
    MigrationStarted,
    /// A single migration was applied.
    MigrationApplied {
        /// Migration name.
        name: String,
    },
    /// Database migration phase finished.
    MigrationComplete {
        /// Number of migrations applied.
        applied: usize,
        /// Number of migrations skipped (already applied).
        skipped: usize,
    },
    /// The database schema was validated against the application.
    DatabaseValidated,

    /// An MCP server is being started.
    McpServerStarting {
        /// Server name.
        name: String,
        /// Bound port.
        port: u16,
    },
    /// Health check probe was issued for an MCP server.
    McpServerHealthCheck {
        /// Server name.
        name: String,
        /// 1-based attempt counter.
        attempt: u8,
        /// Maximum attempts before giving up.
        max_attempts: u8,
    },
    /// An MCP server reported ready.
    McpServerReady {
        /// Server name.
        name: String,
        /// Bound port.
        port: u16,
        /// Time taken to become ready.
        startup_time: Duration,
        /// Number of tools advertised.
        tools: usize,
    },
    /// An MCP server failed to start.
    McpServerFailed {
        /// Server name.
        name: String,
        /// Failure description.
        error: String,
    },
    /// An MCP server was reconciled away (e.g. removed from config).
    McpServiceCleanup {
        /// Server name.
        name: String,
        /// Why the server was cleaned up.
        reason: String,
    },
    /// MCP reconciliation finished.
    McpReconciliationComplete {
        /// Number of servers running after reconciliation.
        running: usize,
        /// Number of servers required by configuration.
        required: usize,
    },

    /// An agent is being started.
    AgentStarting {
        /// Agent name.
        name: String,
        /// Bound port.
        port: u16,
    },
    /// An agent reported ready.
    AgentReady {
        /// Agent name.
        name: String,
        /// Bound port.
        port: u16,
        /// Time taken to become ready.
        startup_time: Duration,
    },
    /// An agent failed to start.
    AgentFailed {
        /// Agent name.
        name: String,
        /// Failure description.
        error: String,
    },
    /// An agent was reconciled away.
    AgentCleanup {
        /// Agent name.
        name: String,
        /// Why the agent was cleaned up.
        reason: String,
    },
    /// Agent reconciliation finished.
    AgentReconciliationComplete {
        /// Agents currently running.
        running: usize,
        /// Total agents configured.
        total: usize,
    },

    /// Route mounting started.
    RoutesConfiguring,
    /// Route mounting finished.
    RoutesConfigured {
        /// Number of modules contributing routes.
        module_count: usize,
    },
    /// A specific extension route was mounted.
    ExtensionRouteMounted {
        /// Mounting extension name.
        name: String,
        /// Mount path.
        path: String,
        /// Whether the route requires authentication.
        auth_required: bool,
    },
    /// HTTP server is binding to its socket.
    ServerBinding {
        /// Address being bound.
        address: String,
    },
    /// HTTP server is listening for connections.
    ServerListening {
        /// Listening address.
        address: String,
        /// Process id.
        pid: u32,
    },

    /// Scheduler initialization phase began.
    SchedulerInitializing,
    /// A scheduler job was registered.
    SchedulerJobRegistered {
        /// Job name.
        name: String,
        /// Cron-style schedule string.
        schedule: String,
    },
    /// Scheduler is ready.
    SchedulerReady {
        /// Number of jobs registered.
        job_count: usize,
    },
    /// A bootstrap job started running.
    BootstrapJobStarted {
        /// Job name.
        name: String,
    },
    /// A bootstrap job finished.
    BootstrapJobCompleted {
        /// Job name.
        name: String,
        /// Whether the job succeeded.
        success: bool,
        /// Optional human-readable status message.
        message: Option<String>,
    },

    /// Non-fatal warning surfaced during startup.
    Warning {
        /// Warning text.
        message: String,
        /// Optional contextual hint.
        context: Option<String>,
    },
    /// Error event surfaced during startup.
    Error {
        /// Error text.
        message: String,
        /// Whether the error halted startup.
        fatal: bool,
    },
    /// Informational event surfaced during startup.
    Info {
        /// Informational text.
        message: String,
    },

    /// Startup completed successfully.
    StartupComplete {
        /// Total startup duration.
        duration: Duration,
        /// Public API URL.
        api_url: String,
        /// Final per-service summaries.
        services: Vec<ServiceInfo>,
    },
    /// Startup failed.
    StartupFailed {
        /// Failure description.
        error: String,
        /// Time spent before the failure.
        duration: Duration,
    },
}
