//! Extension traits for ergonomically emitting [`StartupEvent`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod impls;

use std::time::Duration;

use super::{ModuleInfo, Phase, ServiceInfo, StartupEvent, StartupEventSender};

fn emit(sender: &StartupEventSender, event: StartupEvent) {
    if let Err(e) = sender.unbounded_send(event) {
        tracing::debug!(error = %e, "Startup event dropped: receiver closed");
    }
}

pub trait StartupEventExt {
    fn phase_started(&self, phase: Phase);
    fn phase_completed(&self, phase: Phase);
    fn phase_failed(&self, phase: Phase, error: impl Into<String>);

    fn port_available(&self, port: u16);
    fn port_conflict(&self, port: u16, pid: u32);
    fn modules_loaded(&self, count: usize, modules: Vec<ModuleInfo>);

    fn mcp_starting(&self, name: impl Into<String>, port: u16);
    fn mcp_health_check(&self, name: impl Into<String>, attempt: u8, max: u8);
    fn mcp_ready(
        &self,
        name: impl Into<String>,
        port: u16,
        startup_time: Duration,
        tools: Option<usize>,
    );
    fn mcp_failed(&self, name: impl Into<String>, error: impl Into<String>);
    fn mcp_service_cleanup(&self, name: impl Into<String>, reason: impl Into<String>);
    fn mcp_reconciliation_complete(&self, running: usize, required: usize);

    fn agent_starting(&self, name: impl Into<String>, port: u16);
    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration);
    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>);
    fn agent_cleanup(&self, name: impl Into<String>, reason: impl Into<String>);

    fn server_listening(&self, address: impl Into<String>, pid: u32);

    fn scheduler_initializing(&self);
    fn scheduler_ready(&self, scheduled: usize, available: usize);
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
