//! Startup event variants emitted by the runtime as it brings services up.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use super::{ModuleInfo, Phase, ServiceInfo};

#[derive(Debug, Clone)]
#[non_exhaustive]
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
        /// `None` when the server's tool list was never enumerated, e.g. an
        /// OAuth-gated server validated by reachability alone.
        tools: Option<usize>,
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
    /// `scheduled` is the number of jobs with a `scheduler.jobs` entry, i.e.
    /// what will actually run; `available` is the number compiled into the
    /// binary and discovered via inventory. The two are reported separately
    /// because `available` alone reads as a deployment's job count when it is
    /// really a build capability.
    SchedulerReady {
        scheduled: usize,
        available: usize,
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
