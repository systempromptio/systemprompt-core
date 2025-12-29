use super::state_types::{DesiredStatus, RuntimeStatus, ServiceAction, ServiceType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedServiceState {
    pub name: String,
    pub service_type: ServiceType,
    pub desired_status: DesiredStatus,
    pub runtime_status: RuntimeStatus,
    pub pid: Option<u32>,
    pub port: u16,
    pub needs_action: ServiceAction,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct VerifiedServiceStateBuilder {
    name: String,
    service_type: ServiceType,
    desired: DesiredStatus,
    runtime: RuntimeStatus,
    port: u16,
    pid: Option<u32>,
    error: Option<String>,
}

impl VerifiedServiceStateBuilder {
    pub const fn new(
        name: String,
        service_type: ServiceType,
        desired: DesiredStatus,
        runtime: RuntimeStatus,
        port: u16,
    ) -> Self {
        Self {
            name,
            service_type,
            desired,
            runtime,
            port,
            pid: None,
            error: None,
        }
    }

    pub const fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    pub fn build(self) -> VerifiedServiceState {
        let action = VerifiedServiceState::determine_action(&self.desired, &self.runtime);
        VerifiedServiceState {
            name: self.name,
            service_type: self.service_type,
            desired_status: self.desired,
            runtime_status: self.runtime,
            pid: self.pid,
            port: self.port,
            needs_action: action,
            error: self.error,
        }
    }
}

impl VerifiedServiceState {
    pub const fn builder(
        name: String,
        service_type: ServiceType,
        desired: DesiredStatus,
        runtime: RuntimeStatus,
        port: u16,
    ) -> VerifiedServiceStateBuilder {
        VerifiedServiceStateBuilder::new(name, service_type, desired, runtime, port)
    }

    const fn determine_action(desired: &DesiredStatus, runtime: &RuntimeStatus) -> ServiceAction {
        match (desired, runtime) {
            (DesiredStatus::Enabled, RuntimeStatus::Running | RuntimeStatus::Starting) => {
                ServiceAction::None
            },
            (DesiredStatus::Enabled, RuntimeStatus::Stopped) => ServiceAction::Start,
            (DesiredStatus::Enabled, RuntimeStatus::Crashed | RuntimeStatus::Orphaned) => {
                ServiceAction::Restart
            },
            (DesiredStatus::Disabled, RuntimeStatus::Running | RuntimeStatus::Starting) => {
                ServiceAction::Stop
            },
            (DesiredStatus::Disabled, RuntimeStatus::Stopped | RuntimeStatus::Crashed) => {
                ServiceAction::CleanupDb
            },
            (DesiredStatus::Disabled, RuntimeStatus::Orphaned) => ServiceAction::CleanupProcess,
        }
    }

    pub const fn is_healthy(&self) -> bool {
        matches!(
            (&self.desired_status, &self.runtime_status),
            (
                DesiredStatus::Enabled,
                RuntimeStatus::Running | RuntimeStatus::Starting
            )
        )
    }

    pub const fn needs_attention(&self) -> bool {
        !matches!(self.needs_action, ServiceAction::None)
    }

    pub const fn status_display(&self) -> &'static str {
        match self.runtime_status {
            RuntimeStatus::Running => "running",
            RuntimeStatus::Starting => "starting",
            RuntimeStatus::Stopped => "stopped",
            RuntimeStatus::Crashed => "crashed",
            RuntimeStatus::Orphaned => "orphaned",
        }
    }

    pub const fn action_display(&self) -> &'static str {
        match self.needs_action {
            ServiceAction::None => "none",
            ServiceAction::Start => "start",
            ServiceAction::Stop => "stop",
            ServiceAction::Restart => "restart",
            ServiceAction::CleanupDb => "cleanup-db",
            ServiceAction::CleanupProcess => "cleanup-process",
        }
    }
}
