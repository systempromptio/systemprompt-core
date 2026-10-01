//! Service-layer error type, distinct from the crate-public [`AgentError`]:
//! it models failures internal to runtime services and converts into
//! `AgentError` at the crate boundary.
//!
//! [`AgentError`]: crate::error::AgentError
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{BoxedSource, RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentServiceError {
    #[error("repository operation failed: {0}")]
    Repository(#[from] RepositoryError),

    #[error("agent operation failed: {0}")]
    Agent(#[from] crate::error::AgentError),

    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("ai inference failed: {0}")]
    AiInference(#[from] systemprompt_models::errors::AiInferenceError),

    #[error("mcp registry failed: {0}")]
    McpRegistry(#[from] systemprompt_models::errors::McpRegistryError),

    #[error("validation failed for {field}: {source}")]
    Validation {
        field: &'static str,
        #[source]
        source: BoxedSource,
    },

    #[error("{context}: {source}")]
    Operation {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("stream consumer closed before the task finished")]
    StreamClosed,

    #[error("task was cancelled before it finished")]
    TaskCancelled,

    #[error("skill {skill_id} is managed but withheld ({reason})")]
    SkillWithheld {
        skill_id: systemprompt_identifiers::SkillId,
        reason: &'static str,
    },

    #[error("skill {skill_id} could not be resolved: {source}")]
    SkillSource {
        skill_id: systemprompt_identifiers::SkillId,
        #[source]
        source: systemprompt_traits::ManagedSkillResolverError,
    },
}

impl AgentServiceError {
    pub fn operation<E>(context: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Operation {
            context: context.into(),
            source: Box::new(source),
        }
    }

    pub fn validation<E>(field: &'static str, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Validation {
            field,
            source: Box::new(source),
        }
    }
}

impl From<sqlx::Error> for AgentServiceError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(err.into())
    }
}

pub type Result<T> = std::result::Result<T, AgentServiceError>;
