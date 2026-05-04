//! [`VerifiedServiceState`] — a snapshot of one service's desired vs runtime
//! status, paired with the [`ServiceAction`] required to converge them.

use super::state_types::{DesiredStatus, RuntimeStatus, ServiceAction, ServiceType};
use serde::{Deserialize, Serialize};

/// Snapshot of one service's desired vs runtime state plus required action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedServiceState {
    /// Service identifier as recorded in config and the `services` table.
    pub name: String,
    /// Service-type discriminator (mcp, agent, …).
    pub service_type: ServiceType,
    /// Whether the service should be running per its configuration.
    pub desired_status: DesiredStatus,
    /// Currently observed runtime status.
    pub runtime_status: RuntimeStatus,
    /// PID of the running process, when known.
    pub pid: Option<u32>,
    /// TCP port the service is bound to.
    pub port: u16,
    /// Action the reconciler must take to converge state.
    pub needs_action: ServiceAction,
    /// Optional error message captured during verification.
    pub error: Option<String>,
}

/// Builder for [`VerifiedServiceState`] used by the state manager.
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
    /// Construct a new builder with the mandatory fields.
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

    /// Attach a PID to the snapshot.
    pub const fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    /// Attach an error message captured during verification.
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Finalise the builder, computing the required [`ServiceAction`].
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
    /// Construct a new [`VerifiedServiceStateBuilder`] for fluent assembly.
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

    /// Return whether this service is in a healthy steady state.
    pub const fn is_healthy(&self) -> bool {
        matches!(
            (&self.desired_status, &self.runtime_status),
            (
                DesiredStatus::Enabled,
                RuntimeStatus::Running | RuntimeStatus::Starting
            )
        )
    }

    /// Return whether the reconciler must act on this snapshot.
    pub const fn needs_attention(&self) -> bool {
        !matches!(self.needs_action, ServiceAction::None)
    }

    /// Stable display string for the runtime status.
    pub const fn status_display(&self) -> &'static str {
        match self.runtime_status {
            RuntimeStatus::Running => "running",
            RuntimeStatus::Starting => "starting",
            RuntimeStatus::Stopped => "stopped",
            RuntimeStatus::Crashed => "crashed",
            RuntimeStatus::Orphaned => "orphaned",
        }
    }

    /// Stable display string for the required action.
    pub const fn action_display(&self) -> &'static str {
        match self.needs_action {
            ServiceAction::None => "-",
            ServiceAction::Start => "start",
            ServiceAction::Stop => "stop",
            ServiceAction::Restart => "restart",
            ServiceAction::CleanupDb => "cleanup-db",
            ServiceAction::CleanupProcess => "cleanup-process",
        }
    }
}
