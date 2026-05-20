//! `Option<&StartupEventSender>` mirror of [`StartupEventExt`].

use std::time::Duration;

use super::ext::StartupEventExt;
use super::{ModuleInfo, Phase, ServiceInfo, StartupEventSender};


pub trait OptionalStartupEventExt {
    fn phase_started(&self, phase: Phase);
    fn phase_completed(&self, phase: Phase);
    fn phase_failed(&self, phase: Phase, error: impl Into<String>);

    fn port_available(&self, port: u16);
    fn port_conflict(&self, port: u16, pid: u32);
    fn modules_loaded(&self, count: usize, modules: Vec<ModuleInfo>);

    fn mcp_starting(&self, name: impl Into<String>, port: u16);
    fn mcp_health_check(&self, name: impl Into<String>, attempt: u8, max: u8);
    fn mcp_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration, tools: usize);
    fn mcp_failed(&self, name: impl Into<String>, error: impl Into<String>);
    fn mcp_service_cleanup(&self, name: impl Into<String>, reason: impl Into<String>);
    fn mcp_reconciliation_complete(&self, running: usize, required: usize);

    fn agent_starting(&self, name: impl Into<String>, port: u16);
    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration);
    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>);
    fn agent_cleanup(&self, name: impl Into<String>, reason: impl Into<String>);

    fn server_listening(&self, address: impl Into<String>, pid: u32);

    fn scheduler_initializing(&self);
    fn scheduler_ready(&self, job_count: usize);
    fn bootstrap_job_started(&self, name: impl Into<String>);
    fn bootstrap_job_completed(
        &self,
        name: impl Into<String>,
        success: bool,
        message: Option<String>,
    );

    fn warning(&self, message: impl Into<String>);
    fn warning_with_context(&self, message: impl Into<String>, context: impl Into<String>);
    fn info(&self, message: impl Into<String>);
    fn error(&self, message: impl Into<String>, fatal: bool);

    fn startup_complete(
        &self,
        duration: Duration,
        api_url: impl Into<String>,
        services: Vec<ServiceInfo>,
    );
}

impl OptionalStartupEventExt for Option<&StartupEventSender> {
    fn phase_started(&self, phase: Phase) {
        if let Some(s) = self {
            s.phase_started(phase);
        }
    }

    fn phase_completed(&self, phase: Phase) {
        if let Some(s) = self {
            s.phase_completed(phase);
        }
    }

    fn phase_failed(&self, phase: Phase, error: impl Into<String>) {
        if let Some(s) = self {
            s.phase_failed(phase, error);
        }
    }

    fn port_available(&self, port: u16) {
        if let Some(s) = self {
            s.port_available(port);
        }
    }

    fn port_conflict(&self, port: u16, pid: u32) {
        if let Some(s) = self {
            s.port_conflict(port, pid);
        }
    }

    fn modules_loaded(&self, count: usize, modules: Vec<ModuleInfo>) {
        if let Some(s) = self {
            s.modules_loaded(count, modules);
        }
    }

    fn mcp_starting(&self, name: impl Into<String>, port: u16) {
        if let Some(s) = self {
            s.mcp_starting(name, port);
        }
    }

    fn mcp_health_check(&self, name: impl Into<String>, attempt: u8, max: u8) {
        if let Some(s) = self {
            s.mcp_health_check(name, attempt, max);
        }
    }

    fn mcp_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration, tools: usize) {
        if let Some(s) = self {
            s.mcp_ready(name, port, startup_time, tools);
        }
    }

    fn mcp_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        if let Some(s) = self {
            s.mcp_failed(name, error);
        }
    }

    fn mcp_service_cleanup(&self, name: impl Into<String>, reason: impl Into<String>) {
        if let Some(s) = self {
            s.mcp_service_cleanup(name, reason);
        }
    }

    fn mcp_reconciliation_complete(&self, running: usize, required: usize) {
        if let Some(s) = self {
            s.mcp_reconciliation_complete(running, required);
        }
    }

    fn agent_starting(&self, name: impl Into<String>, port: u16) {
        if let Some(s) = self {
            s.agent_starting(name, port);
        }
    }

    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration) {
        if let Some(s) = self {
            s.agent_ready(name, port, startup_time);
        }
    }

    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        if let Some(s) = self {
            s.agent_failed(name, error);
        }
    }

    fn agent_cleanup(&self, name: impl Into<String>, reason: impl Into<String>) {
        if let Some(s) = self {
            s.agent_cleanup(name, reason);
        }
    }

    fn server_listening(&self, address: impl Into<String>, pid: u32) {
        if let Some(s) = self {
            s.server_listening(address, pid);
        }
    }

    fn scheduler_initializing(&self) {
        if let Some(s) = self {
            s.scheduler_initializing();
        }
    }

    fn scheduler_ready(&self, job_count: usize) {
        if let Some(s) = self {
            s.scheduler_ready(job_count);
        }
    }

    fn bootstrap_job_started(&self, name: impl Into<String>) {
        if let Some(s) = self {
            s.bootstrap_job_started(name);
        }
    }

    fn bootstrap_job_completed(
        &self,
        name: impl Into<String>,
        success: bool,
        message: Option<String>,
    ) {
        if let Some(s) = self {
            s.bootstrap_job_completed(name, success, message);
        }
    }

    fn warning(&self, message: impl Into<String>) {
        if let Some(s) = self {
            s.warning(message);
        }
    }

    fn warning_with_context(&self, message: impl Into<String>, context: impl Into<String>) {
        if let Some(s) = self {
            s.warning_with_context(message, context);
        }
    }

    fn info(&self, message: impl Into<String>) {
        if let Some(s) = self {
            s.info(message);
        }
    }

    fn error(&self, message: impl Into<String>, fatal: bool) {
        if let Some(s) = self {
            s.error(message, fatal);
        }
    }

    fn startup_complete(
        &self,
        duration: Duration,
        api_url: impl Into<String>,
        services: Vec<ServiceInfo>,
    ) {
        if let Some(s) = self {
            s.startup_complete(duration, api_url, services);
        }
    }
}
