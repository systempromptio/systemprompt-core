//! Typed error hierarchy for the `systemprompt-agent` crate.
//!
//! Public APIs return concrete `thiserror`-derived enums instead of
//! `anyhow::Error` so that downstream callers can match on error variants
//! without string parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{McpToolName, TaskId};
use systemprompt_models::StepId;
use systemprompt_traits::{BoxedSource, MetadataValidationError, RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Invalid tool response schema: expected {expected}, found keys: {actual_keys:?}")]
    InvalidSchema {
        expected: &'static str,
        actual_keys: Vec<String>,
        #[source]
        source: serde_json::Error,
    },

    #[error("Metadata validation error: {0}")]
    MetadataValidation(#[from] MetadataValidationError),

    #[error(
        "tool {tool_name} declares no x-artifact-type; add x-artifact-type to the tool output or \
         its output schema"
    )]
    MissingArtifactType { tool_name: McpToolName },

    #[error("artifact must be a JSON object, found {found}")]
    ArtifactNotObject { found: &'static str },
}

/// The execution-step row(s) a failed write was addressing: one step, or
/// every in-progress step of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStepTarget {
    Step(StepId),
    Task(TaskId),
}

impl std::fmt::Display for ExecutionStepTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Step(step_id) => write!(f, "step {step_id}"),
            Self::Task(task_id) => write!(f, "steps of task {task_id}"),
        }
    }
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Artifact error: {0}")]
    Artifact(#[from] ArtifactError),

    #[error("repository: {0}")]
    Repository(#[from] RepositoryError),

    #[error("execution {target} could not be written: {source}")]
    ExecutionStepWrite {
        target: ExecutionStepTarget,
        #[source]
        source: RepositoryError,
    },

    #[error("config: cors_allowed_origins must contain at least one valid origin")]
    EmptyCorsAllowlist,

    #[error("config: {context}: {source}")]
    InvalidConfig {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("agent not found: {0}")]
    NotFound(String),

    #[error("No available ports in range {min}-{max}")]
    NoAvailablePort { min: u16, max: u16 },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("services config: {0}")]
    ServicesConfig(#[from] systemprompt_loader::ConfigLoadError),
}

impl AgentError {
    pub fn step_write(step_id: &StepId, source: impl Into<RepositoryError>) -> Self {
        Self::ExecutionStepWrite {
            target: ExecutionStepTarget::Step(step_id.clone()),
            source: source.into(),
        }
    }

    pub fn task_steps_write(task_id: &TaskId, source: impl Into<RepositoryError>) -> Self {
        Self::ExecutionStepWrite {
            target: ExecutionStepTarget::Task(task_id.clone()),
            source: source.into(),
        }
    }

    pub fn invalid_config<E>(context: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::InvalidConfig {
            context: context.into(),
            source: Box::new(source),
        }
    }
}

impl From<sqlx::Error> for AgentError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(err.into())
    }
}

pub type AgentResult<T> = Result<T, AgentError>;
