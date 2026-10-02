//! Supervision of agent worker processes: lifecycle, monitoring, and
//! reconciliation.
//!
//! This module groups the services that keep the database's view of running
//! agents consistent with the OS process table. [`AgentOrchestrator`] is the
//! top-level facade; the submodules cover process lifecycle, health
//! checks, drift reconciliation, port allocation, and the low-level process
//! primitives. [`AgentStatus`] is the shared status model and
//! [`OrchestrationError`] the unified error type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod database;
pub mod lifecycle;
pub mod monitor;
pub mod orchestrator;
pub mod port_service;
pub mod process;
pub mod reconciler;

pub use orchestrator::{AgentInfo, AgentOrchestrator};
pub use port_service::PortService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Running {
        pid: u32,
        port: u16,
    },
    Failed {
        reason: String,
        last_attempt: Option<String>,
        retry_count: u32,
    },
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<String>,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationReport {
    pub const fn new() -> Self {
        Self {
            valid: true,
            issues: Vec::new(),
        }
    }

    pub fn with_issue(issue: String) -> Self {
        Self {
            valid: false,
            issues: vec![issue],
        }
    }

    pub fn add_issue(&mut self, issue: String) {
        self.valid = false;
        self.issues.push(issue);
    }
}

use crate::services::shared::AgentServiceError;
use systemprompt_identifiers::AgentName;
use systemprompt_traits::{BoxedSource, RepositoryError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestrationError {
    #[error("Agent {0} not found")]
    AgentNotFound(String),

    #[error("Agent {0} already running")]
    AgentAlreadyRunning(String),

    #[error("Process spawn failed: {0}")]
    ProcessSpawnFailed(String),

    #[error("Process spawn failed: {context}: {source}")]
    Spawn {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("repository: {0}")]
    Repository(#[from] RepositoryError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Health check timeout for agent {0}")]
    HealthCheckTimeout(String),

    #[error("Failed to load agent registry: {0}")]
    Registry(#[source] crate::error::AgentError),

    #[error("agent: {0}")]
    Agent(#[from] crate::error::AgentError),

    #[error("Service error: {0}")]
    AgentService(#[from] AgentServiceError),

    #[error("process supervision: {0}")]
    Supervision(#[from] systemprompt_loader::subprocess::SupervisionError),

    #[error(
        "port {port} for agent {agent} is held by process {pid}, which this installation did \
         not spawn; stop it or choose a different port"
    )]
    PortHeldByForeignProcess {
        port: u16,
        pid: u32,
        agent: AgentName,
    },
}

impl OrchestrationError {
    pub fn spawn<E>(context: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Spawn {
            context: context.into(),
            source: Box::new(source),
        }
    }
}

impl From<sqlx::Error> for OrchestrationError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(err.into())
    }
}

pub type OrchestrationResult<T> = Result<T, OrchestrationError>;
