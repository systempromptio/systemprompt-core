//! Extension trait for ergonomically emitting [`StartupEvent`]s.
//!
//! [`StartupEventExt`] is implemented for [`StartupEventSender`] and for
//! `Option<&StartupEventSender>`, so a caller that may run without a startup
//! listener (CLI one-shots, tests) uses the same methods; every event
//! becomes a no-op when the option is `None`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use super::{ModuleInfo, Phase, ServiceInfo, StartupEvent, StartupEventSender};

pub trait StartupEventExt {
    fn sender(&self) -> Option<&StartupEventSender>;

    fn emit(&self, event: StartupEvent) {
        let Some(sender) = self.sender() else {
            return;
        };
        if let Err(e) = sender.unbounded_send(event) {
            tracing::debug!(error = %e, "Startup event dropped: receiver closed");
        }
    }

    fn phase_started(&self, phase: Phase) {
        self.emit(StartupEvent::PhaseStarted { phase });
    }

    fn phase_completed(&self, phase: Phase) {
        self.emit(StartupEvent::PhaseCompleted { phase });
    }

    fn phase_failed(&self, phase: Phase, error: impl Into<String>) {
        self.emit(StartupEvent::PhaseFailed {
            phase,
            error: error.into(),
        });
    }

    fn port_available(&self, port: u16) {
        self.emit(StartupEvent::PortAvailable { port });
    }

    fn port_conflict(&self, port: u16, pid: u32) {
        self.emit(StartupEvent::PortConflict { port, pid });
    }

    fn modules_loaded(&self, count: usize, modules: Vec<ModuleInfo>) {
        self.emit(StartupEvent::ModulesLoaded { count, modules });
    }

    fn mcp_starting(&self, name: impl Into<String>, port: u16) {
        self.emit(StartupEvent::McpServerStarting {
            name: name.into(),
            port,
        });
    }

    fn mcp_health_check(&self, name: impl Into<String>, attempt: u8, max: u8) {
        self.emit(StartupEvent::McpServerHealthCheck {
            name: name.into(),
            attempt,
            max_attempts: max,
        });
    }

    fn mcp_ready(
        &self,
        name: impl Into<String>,
        port: u16,
        startup_time: Duration,
        tools: Option<usize>,
    ) {
        self.emit(StartupEvent::McpServerReady {
            name: name.into(),
            port,
            startup_time,
            tools,
        });
    }

    fn mcp_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        self.emit(StartupEvent::McpServerFailed {
            name: name.into(),
            error: error.into(),
        });
    }

    fn agent_starting(&self, name: impl Into<String>, port: u16) {
        self.emit(StartupEvent::AgentStarting {
            name: name.into(),
            port,
        });
    }

    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration) {
        self.emit(StartupEvent::AgentReady {
            name: name.into(),
            port,
            startup_time,
        });
    }

    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        self.emit(StartupEvent::AgentFailed {
            name: name.into(),
            error: error.into(),
        });
    }

    fn agent_cleanup(&self, name: impl Into<String>, reason: impl Into<String>) {
        self.emit(StartupEvent::AgentCleanup {
            name: name.into(),
            reason: reason.into(),
        });
    }

    fn mcp_service_cleanup(&self, name: impl Into<String>, reason: impl Into<String>) {
        self.emit(StartupEvent::McpServiceCleanup {
            name: name.into(),
            reason: reason.into(),
        });
    }

    fn mcp_reconciliation_complete(&self, running: usize, required: usize) {
        self.emit(StartupEvent::McpReconciliationComplete { running, required });
    }

    fn scheduler_initializing(&self) {
        self.emit(StartupEvent::SchedulerInitializing);
    }

    fn scheduler_ready(&self, scheduled: usize, available: usize) {
        self.emit(StartupEvent::SchedulerReady {
            scheduled,
            available,
        });
    }

    fn bootstrap_job_started(&self, name: impl Into<String>) {
        self.emit(StartupEvent::BootstrapJobStarted { name: name.into() });
    }

    fn bootstrap_job_completed(
        &self,
        name: impl Into<String>,
        success: bool,
        message: Option<String>,
    ) {
        self.emit(StartupEvent::BootstrapJobCompleted {
            name: name.into(),
            success,
            message,
        });
    }

    fn server_listening(&self, address: impl Into<String>, pid: u32) {
        self.emit(StartupEvent::ServerListening {
            address: address.into(),
            pid,
        });
    }

    fn warning(&self, message: impl Into<String>) {
        self.emit(StartupEvent::Warning {
            message: message.into(),
            context: None,
        });
    }

    fn warning_with_context(&self, message: impl Into<String>, context: impl Into<String>) {
        self.emit(StartupEvent::Warning {
            message: message.into(),
            context: Some(context.into()),
        });
    }

    fn info(&self, message: impl Into<String>) {
        self.emit(StartupEvent::Info {
            message: message.into(),
        });
    }

    fn error(&self, message: impl Into<String>, fatal: bool) {
        self.emit(StartupEvent::Error {
            message: message.into(),
            fatal,
        });
    }

    fn startup_complete(
        &self,
        duration: Duration,
        api_url: impl Into<String>,
        services: Vec<ServiceInfo>,
    ) {
        self.emit(StartupEvent::StartupComplete {
            duration,
            api_url: api_url.into(),
            services,
        });
    }
}

impl StartupEventExt for StartupEventSender {
    fn sender(&self) -> Option<&StartupEventSender> {
        Some(self)
    }
}

impl StartupEventExt for Option<&StartupEventSender> {
    fn sender(&self) -> Option<&StartupEventSender> {
        *self
    }
}
