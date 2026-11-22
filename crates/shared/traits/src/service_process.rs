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

/// One supervised process as its set reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceProcessStatus {
    pub name: ServiceName,
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceProcessError {
    #[error("no {kind:?} named {name}")]
    Unknown {
        kind: ServiceProcessKind,
        name: ServiceName,
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

    async fn statuses(&self) -> Result<Vec<ServiceProcessStatus>, ServiceProcessError>;

    async fn start(&self, name: Option<&ServiceName>) -> Result<(), ServiceProcessError>;

    async fn stop(&self, name: Option<&ServiceName>) -> Result<(), ServiceProcessError>;

    async fn restart(&self, name: &ServiceName) -> Result<(), ServiceProcessError>;
}

pub type DynServiceProcessSet = Arc<dyn ServiceProcessSet>;
