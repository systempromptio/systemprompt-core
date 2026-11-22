//! [`ServiceProcessSet`]: one kind of process the platform supervises (the
//! agent processes, the managed MCP servers), driven by the kernel without
//! naming the member that owns it.
//!
//! Starting a process mints its credentials, so a set is a standing
//! capability: each member contributes its set through the sealed, core-first
//! `ServiceProcessSlot` (systemprompt-extension), which only kernel code
//! (`infra services`, the API lifecycle) reads. A member command that shows
//! process status reads its own member's open status view instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::ServiceName;

use crate::BoxedSource;

/// The kind of process a set supervises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceProcessKind {
    Agent,
    McpServer,
}

/// The health a set reports for one process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl ServiceHealth {
    #[must_use]
    pub const fn is_up(self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

/// What one kind of process reports beyond the common fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceDetail {
    Agent {
        enabled: bool,
        failure: Option<String>,
    },
    Mcp {
        remote: bool,
        endpoint: Option<String>,
        tools_count: Option<usize>,
        latency_ms: Option<u32>,
        auth_required: bool,
    },
}

/// One supervised process as its set reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProcessStatus {
    pub name: ServiceName,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub health: ServiceHealth,
    pub detail: ServiceDetail,
}

/// How deep `statuses` looks: the configured members only, or with a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusDepth {
    Listed,
    Probed,
}

/// `Disable` stops a process and marks it disabled; `Remove` also drops its
/// service record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopMode {
    Disable,
    Remove,
}

/// `Rebuild` rebuilds the process's binary before restarting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartMode {
    Restart,
    Rebuild,
}

/// The outcome of starting or restarting one process: the typed service name
/// it runs under, and whether it did.
#[derive(Debug)]
pub struct ServiceRun {
    pub name: ServiceName,
    pub outcome: Result<(), ServiceProcessError>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceProcessError {
    #[error("no {kind:?} named {name}")]
    Unknown {
        kind: ServiceProcessKind,
        name: ServiceName,
    },
    #[error("{kind:?} processes are not supervised on this host")]
    Unavailable { kind: ServiceProcessKind },
    #[error("{kind:?} does not support {operation}")]
    Unsupported {
        kind: ServiceProcessKind,
        operation: &'static str,
    },
    #[error("the {kind:?} process set failed")]
    Failed {
        kind: ServiceProcessKind,
        #[source]
        source: BoxedSource,
    },
}

/// Status and control of one kind of supervised process.
///
/// `None` for a name addresses every process of the set. Injected as
/// `dyn ServiceProcessSet`, hence `#[async_trait]`.
#[async_trait]
pub trait ServiceProcessSet: Send + Sync {
    fn kind(&self) -> ServiceProcessKind;

    async fn statuses(
        &self,
        depth: StatusDepth,
    ) -> Result<Vec<ServiceProcessStatus>, ServiceProcessError>;

    async fn start(
        &self,
        name: Option<&ServiceName>,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError>;

    async fn stop(
        &self,
        name: Option<&ServiceName>,
        mode: StopMode,
    ) -> Result<(), ServiceProcessError>;

    async fn restart(
        &self,
        name: Option<&ServiceName>,
        mode: RestartMode,
    ) -> Result<Vec<ServiceRun>, ServiceProcessError>;
}

pub type DynServiceProcessSet = Arc<dyn ServiceProcessSet>;
