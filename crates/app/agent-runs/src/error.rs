//! Typed error boundary for the `systemprompt-agent-runs` crate.
//!
//! [`AgentRunsError`] is returned from every public, non-trait signature. Each
//! variant carries its cause through `#[from]` or `#[source]`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_traits::RepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentRunsError {
    #[error("unknown headless host '{value}'")]
    UnknownHost { value: String },

    #[error("unknown permission mode '{value}'")]
    UnknownPermissionMode { value: String },

    #[error("unknown run event kind '{value}'")]
    UnknownEventKind { value: String },

    #[error("unknown run status '{value}'")]
    UnknownStatus { value: String },

    #[error("unknown error class '{value}'")]
    UnknownErrorClass { value: String },

    #[error("host {host} cannot run headless: {reason}")]
    UnsupportedHost {
        host: crate::request::HostKind,
        reason: &'static str,
    },

    #[error("run directory {path}: {source}", path = path.display())]
    Layout {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("run API key {step}: {source}")]
    ApiKey {
        step: &'static str,
        #[source]
        source: systemprompt_users::UserError,
    },

    #[error("run user store ({step}): {source}")]
    UserStore {
        step: &'static str,
        #[source]
        source: systemprompt_users::UserError,
    },

    #[error("run user {user_id} not found")]
    UserNotFound {
        user_id: systemprompt_identifiers::UserId,
    },

    #[error("run gateway session: {source}")]
    Session {
        #[source]
        source: systemprompt_oauth::services::SessionCreationError,
    },

    #[error("run hook token: {source}")]
    PluginToken {
        #[source]
        source: systemprompt_oauth::OauthError,
    },

    #[error("run credential ttl out of range: {source}")]
    CredentialTtl {
        #[source]
        source: chrono::OutOfRangeError,
    },

    #[error("run budget {usd} USD is not a representable non-negative amount")]
    InvalidBudget { usd: f64 },

    #[error("governance hooks file {path}: {source}", path = path.display())]
    HooksWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("spawn {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),
}

impl From<sqlx::Error> for AgentRunsError {
    fn from(error: sqlx::Error) -> Self {
        Self::Repository(RepositoryError::from(error))
    }
}

pub type AgentRunsResult<T> = Result<T, AgentRunsError>;
