//! `StartupEventExt` implementation for the startup event sender.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use super::{StartupEventExt, emit};
use crate::startup_events::{ModuleInfo, Phase, ServiceInfo, StartupEvent, StartupEventSender};

impl StartupEventExt for StartupEventSender {
    fn phase_started(&self, phase: Phase) {
        emit(self, StartupEvent::PhaseStarted { phase });
    }

    fn phase_completed(&self, phase: Phase) {
        emit(self, StartupEvent::PhaseCompleted { phase });
    }

    fn phase_failed(&self, phase: Phase, error: impl Into<String>) {
        emit(
            self,
            StartupEvent::PhaseFailed {
                phase,
                error: error.into(),
            },
        );
    }

    fn port_available(&self, port: u16) {
        emit(self, StartupEvent::PortAvailable { port });
    }

    fn port_conflict(&self, port: u16, pid: u32) {
        emit(self, StartupEvent::PortConflict { port, pid });
    }

    fn modules_loaded(&self, count: usize, modules: Vec<ModuleInfo>) {
        emit(self, StartupEvent::ModulesLoaded { count, modules });
    }

    fn mcp_starting(&self, name: impl Into<String>, port: u16) {
        emit(
            self,
            StartupEvent::McpServerStarting {
                name: name.into(),
                port,
            },
        );
    }

    fn mcp_health_check(&self, name: impl Into<String>, attempt: u8, max: u8) {
        emit(
            self,
            StartupEvent::McpServerHealthCheck {
                name: name.into(),
                attempt,
                max_attempts: max,
            },
        );
    }

    fn mcp_ready(
        &self,
        name: impl Into<String>,
        port: u16,
        startup_time: Duration,
        tools: Option<usize>,
    ) {
        emit(
            self,
            StartupEvent::McpServerReady {
                name: name.into(),
                port,
                startup_time,
                tools,
            },
        );
    }

    fn mcp_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        emit(
            self,
            StartupEvent::McpServerFailed {
                name: name.into(),
                error: error.into(),
            },
        );
    }

    fn agent_starting(&self, name: impl Into<String>, port: u16) {
        emit(
            self,
            StartupEvent::AgentStarting {
                name: name.into(),
                port,
            },
        );
    }

    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration) {
        emit(
            self,
            StartupEvent::AgentReady {
                name: name.into(),
                port,
                startup_time,
            },
        );
    }

    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        emit(
            self,
            StartupEvent::AgentFailed {
                name: name.into(),
                error: error.into(),
            },
        );
    }

    fn agent_cleanup(&self, name: impl Into<String>, reason: impl Into<String>) {
        emit(
            self,
            StartupEvent::AgentCleanup {
                name: name.into(),
                reason: reason.into(),
            },
        );
    }

    fn mcp_service_cleanup(&self, name: impl Into<String>, reason: impl Into<String>) {
        emit(
            self,
            StartupEvent::McpServiceCleanup {
                name: name.into(),
                reason: reason.into(),
            },
        );
    }

    fn mcp_reconciliation_complete(&self, running: usize, required: usize) {
        emit(
            self,
            StartupEvent::McpReconciliationComplete { running, required },
        );
    }

    fn scheduler_initializing(&self) {
        emit(self, StartupEvent::SchedulerInitializing);
    }

    fn scheduler_ready(&self, scheduled: usize, available: usize) {
        emit(
            self,
            StartupEvent::SchedulerReady {
                scheduled,
                available,
            },
        );
    }

    fn bootstrap_job_started(&self, name: impl Into<String>) {
        emit(
            self,
            StartupEvent::BootstrapJobStarted { name: name.into() },
        );
    }

    fn bootstrap_job_completed(
        &self,
        name: impl Into<String>,
        success: bool,
        message: Option<String>,
    ) {
        emit(
            self,
            StartupEvent::BootstrapJobCompleted {
                name: name.into(),
                success,
                message,
            },
        );
    }

    fn server_listening(&self, address: impl Into<String>, pid: u32) {
        emit(
            self,
            StartupEvent::ServerListening {
                address: address.into(),
                pid,
            },
        );
    }

    fn warning(&self, message: impl Into<String>) {
        emit(
            self,
            StartupEvent::Warning {
                message: message.into(),
                context: None,
            },
        );
    }

    fn warning_with_context(&self, message: impl Into<String>, context: impl Into<String>) {
        emit(
            self,
            StartupEvent::Warning {
                message: message.into(),
                context: Some(context.into()),
            },
        );
    }

    fn info(&self, message: impl Into<String>) {
        emit(
            self,
            StartupEvent::Info {
                message: message.into(),
            },
        );
    }

    fn error(&self, message: impl Into<String>, fatal: bool) {
        emit(
            self,
            StartupEvent::Error {
                message: message.into(),
                fatal,
            },
        );
    }

    fn startup_complete(
        &self,
        duration: Duration,
        api_url: impl Into<String>,
        services: Vec<ServiceInfo>,
    ) {
        emit(
            self,
            StartupEvent::StartupComplete {
                duration,
                api_url: api_url.into(),
                services,
            },
        );
    }
}
