//! Extension traits for startup event senders.

use std::time::Duration;

use super::{ModuleInfo, Phase, ServiceInfo, StartupEvent, StartupEventSender};

pub trait StartupEventExt {
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

    fn agent_starting(&self, name: impl Into<String>, port: u16);
    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration);
    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>);

    fn server_listening(&self, address: impl Into<String>, pid: u32);

    fn warning(&self, message: impl Into<String>);
    fn info(&self, message: impl Into<String>);

    fn startup_complete(
        &self,
        duration: Duration,
        api_url: impl Into<String>,
        services: Vec<ServiceInfo>,
    );
}

impl StartupEventExt for StartupEventSender {
    fn phase_started(&self, phase: Phase) {
        let _ = self.unbounded_send(StartupEvent::PhaseStarted { phase });
    }

    fn phase_completed(&self, phase: Phase) {
        let _ = self.unbounded_send(StartupEvent::PhaseCompleted { phase });
    }

    fn phase_failed(&self, phase: Phase, error: impl Into<String>) {
        let _ = self.unbounded_send(StartupEvent::PhaseFailed {
            phase,
            error: error.into(),
        });
    }

    fn port_available(&self, port: u16) {
        let _ = self.unbounded_send(StartupEvent::PortAvailable { port });
    }

    fn port_conflict(&self, port: u16, pid: u32) {
        let _ = self.unbounded_send(StartupEvent::PortConflict { port, pid });
    }

    fn modules_loaded(&self, count: usize, modules: Vec<ModuleInfo>) {
        let _ = self.unbounded_send(StartupEvent::ModulesLoaded { count, modules });
    }

    fn mcp_starting(&self, name: impl Into<String>, port: u16) {
        let _ = self.unbounded_send(StartupEvent::McpServerStarting {
            name: name.into(),
            port,
        });
    }

    fn mcp_health_check(&self, name: impl Into<String>, attempt: u8, max: u8) {
        let _ = self.unbounded_send(StartupEvent::McpServerHealthCheck {
            name: name.into(),
            attempt,
            max_attempts: max,
        });
    }

    fn mcp_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration, tools: usize) {
        let _ = self.unbounded_send(StartupEvent::McpServerReady {
            name: name.into(),
            port,
            startup_time,
            tools,
        });
    }

    fn mcp_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        let _ = self.unbounded_send(StartupEvent::McpServerFailed {
            name: name.into(),
            error: error.into(),
        });
    }

    fn agent_starting(&self, name: impl Into<String>, port: u16) {
        let _ = self.unbounded_send(StartupEvent::AgentStarting {
            name: name.into(),
            port,
        });
    }

    fn agent_ready(&self, name: impl Into<String>, port: u16, startup_time: Duration) {
        let _ = self.unbounded_send(StartupEvent::AgentReady {
            name: name.into(),
            port,
            startup_time,
        });
    }

    fn agent_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        let _ = self.unbounded_send(StartupEvent::AgentFailed {
            name: name.into(),
            error: error.into(),
        });
    }

    fn server_listening(&self, address: impl Into<String>, pid: u32) {
        let _ = self.unbounded_send(StartupEvent::ServerListening {
            address: address.into(),
            pid,
        });
    }

    fn warning(&self, message: impl Into<String>) {
        let _ = self.unbounded_send(StartupEvent::Warning {
            message: message.into(),
            context: None,
        });
    }

    fn info(&self, message: impl Into<String>) {
        let _ = self.unbounded_send(StartupEvent::Info {
            message: message.into(),
        });
    }

    fn startup_complete(
        &self,
        duration: Duration,
        api_url: impl Into<String>,
        services: Vec<ServiceInfo>,
    ) {
        let _ = self.unbounded_send(StartupEvent::StartupComplete {
            duration,
            api_url: api_url.into(),
            services,
        });
    }
}

pub trait OptionalStartupEventExt {
    fn phase_started(&self, phase: Phase);
    fn mcp_starting(&self, name: impl Into<String>, port: u16);
}

impl OptionalStartupEventExt for Option<&StartupEventSender> {
    fn phase_started(&self, phase: Phase) {
        if let Some(sender) = self {
            sender.phase_started(phase);
        }
    }

    fn mcp_starting(&self, name: impl Into<String>, port: u16) {
        if let Some(sender) = self {
            sender.mcp_starting(name, port);
        }
    }
}
