//! Startup event variants.

use std::time::Duration;

use super::{ModuleInfo, Phase, ServiceInfo};

#[derive(Debug, Clone)]
pub enum StartupEvent {
    PhaseStarted {
        phase: Phase,
    },
    PhaseCompleted {
        phase: Phase,
    },
    PhaseFailed {
        phase: Phase,
        error: String,
    },

    PortCheckStarted {
        port: u16,
    },
    PortAvailable {
        port: u16,
    },
    PortConflict {
        port: u16,
        pid: u32,
    },
    PortConflictResolved {
        port: u16,
    },
    ModulesLoaded {
        count: usize,
        modules: Vec<ModuleInfo>,
    },

    MigrationStarted,
    MigrationApplied {
        name: String,
    },
    MigrationComplete {
        applied: usize,
        skipped: usize,
    },
    DatabaseValidated,

    McpServerStarting {
        name: String,
        port: u16,
    },
    McpServerHealthCheck {
        name: String,
        attempt: u8,
        max_attempts: u8,
    },
    McpServerReady {
        name: String,
        port: u16,
        startup_time: Duration,
        tools: usize,
    },
    McpServerFailed {
        name: String,
        error: String,
    },
    McpServiceCleanup {
        name: String,
        reason: String,
    },
    McpReconciliationComplete {
        running: usize,
        required: usize,
    },

    AgentStarting {
        name: String,
        port: u16,
    },
    AgentReady {
        name: String,
        port: u16,
        startup_time: Duration,
    },
    AgentFailed {
        name: String,
        error: String,
    },
    AgentCleanup {
        name: String,
        reason: String,
    },
    AgentReconciliationComplete {
        running: usize,
        total: usize,
    },

    RoutesConfiguring,
    RoutesConfigured {
        module_count: usize,
    },
    ExtensionRouteMounted {
        name: String,
        path: String,
        auth_required: bool,
    },
    ServerBinding {
        address: String,
    },
    ServerListening {
        address: String,
        pid: u32,
    },

    SchedulerInitializing,
    SchedulerJobRegistered {
        name: String,
        schedule: String,
    },
    SchedulerReady {
        job_count: usize,
    },
    BootstrapJobStarted {
        name: String,
    },
    BootstrapJobCompleted {
        name: String,
        success: bool,
        message: Option<String>,
    },

    Warning {
        message: String,
        context: Option<String>,
    },
    Error {
        message: String,
        fatal: bool,
    },
    Info {
        message: String,
    },

    StartupComplete {
        duration: Duration,
        api_url: String,
        services: Vec<ServiceInfo>,
    },
    StartupFailed {
        error: String,
        duration: Duration,
    },
}
