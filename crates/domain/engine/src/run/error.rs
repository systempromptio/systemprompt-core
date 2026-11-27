//! The engine's typed failures, and the coarse kind the shell maps to
//! user-facing text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;
use std::path::PathBuf;

use crate::llm::LlmError;
use crate::store::{MessageId, SessionId, StoreError};
use serde::{Deserialize, Serialize};

use crate::run::governor::GovernError;

/// What class of problem an [`EngineError`] is.
///
/// It chooses the message and the action the shell offers. `NotSignedIn` means
/// no instance is applied and there are no models, so the shell shows
/// its sign-in gate; `ModelNotOffered` means the chosen model is not on the
/// instance's list; `Governance` means the instance refused the call or the
/// response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    NotSignedIn,
    ModelNotOffered,
    InvalidInput,
    NotFound,
    Busy,
    Governance,
    Model,
    ContextOverflow,
    Aborted,
    Config,
    Storage,
    Workspace,
    Unavailable,
    Internal,
}

/// Every way an engine operation can fail.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("no models are on offer; sign in to an instance first")]
    NoModels,
    #[error("model {model} is not offered by the instance")]
    ModelNotOffered { model: String },
    #[error("agent {name} does not exist")]
    UnknownAgent { name: String },
    #[error("command /{name} does not exist")]
    UnknownCommand { name: String },
    #[error("session {id} does not exist")]
    SessionNotFound { id: SessionId },
    #[error("message {id} is not part of session {session}")]
    MessageNotFound { session: SessionId, id: MessageId },
    #[error("session {id} is busy")]
    Busy { id: SessionId },
    #[error("{} is not an absolute directory", .path.display())]
    RelativeDirectory { path: PathBuf },
    #[error("there is nothing to undo or redo")]
    NothingToRevert,
    #[error("the prompt is empty")]
    EmptyPrompt,
    #[error("cannot attach {}", .path.display())]
    Attachment {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the attachment is not a base64 data URL")]
    InvalidDataUrl,
    #[error("the session store failed")]
    Store(#[from] StoreError),
    #[error("the configuration for {} could not be loaded", .dir.display())]
    Config {
        dir: PathBuf,
        #[source]
        source: Box<crate::config::ConfigError>,
    },
    #[error("workspace snapshots failed")]
    Snapshot(#[from] crate::workspace::SnapshotError),
    #[error("the MCP client failed")]
    Mcp(#[from] crate::mcp::McpError),
    #[error("the model call was not authorised")]
    Governance(#[source] GovernError),
    #[error("the instance refused the model response: {reason}")]
    Refused { reason: String },
    #[error("the model call failed")]
    Model(#[source] LlmError),
    #[error("the run was aborted")]
    Aborted,
    #[error("a background task failed")]
    Task(#[source] tokio::task::JoinError),
    #[error("the engine must be created inside a tokio runtime")]
    NoRuntime(#[source] tokio::runtime::TryCurrentError),
    #[error("the engine has shut down")]
    ShutDown,
}

impl EngineError {
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::NoModels => ErrorKind::NotSignedIn,
            Self::ModelNotOffered { .. } => ErrorKind::ModelNotOffered,
            Self::UnknownAgent { .. }
            | Self::UnknownCommand { .. }
            | Self::RelativeDirectory { .. }
            | Self::NothingToRevert
            | Self::EmptyPrompt
            | Self::Attachment { .. }
            | Self::InvalidDataUrl => ErrorKind::InvalidInput,
            Self::SessionNotFound { .. } | Self::MessageNotFound { .. } => ErrorKind::NotFound,
            Self::Busy { .. } => ErrorKind::Busy,
            Self::Store(_) => ErrorKind::Storage,
            Self::Config { .. } => ErrorKind::Config,
            Self::Snapshot(_) => ErrorKind::Workspace,
            Self::Mcp(_) => ErrorKind::Unavailable,
            Self::Governance(error) => governance_kind(error),
            Self::Refused { .. } => ErrorKind::Governance,
            Self::Model(error) => model_kind(error),
            Self::Aborted => ErrorKind::Aborted,
            Self::Task(_) | Self::NoRuntime(_) | Self::ShutDown => ErrorKind::Internal,
        }
    }

    pub fn is_aborted(&self) -> bool {
        self.kind() == ErrorKind::Aborted
    }

    pub fn describe(&self) -> String {
        let mut text = self.to_string();
        let mut source = self.source();
        while let Some(cause) = source {
            text.push_str(": ");
            text.push_str(&cause.to_string());
            source = cause.source();
        }
        text
    }

    pub(crate) fn from_llm(error: LlmError) -> Self {
        match error {
            LlmError::Aborted => Self::Aborted,
            other => Self::Model(other),
        }
    }
}

const fn governance_kind(error: &GovernError) -> ErrorKind {
    match error {
        GovernError::NotSignedIn => ErrorKind::NotSignedIn,
        GovernError::ModelNotOffered { .. } => ErrorKind::ModelNotOffered,
        GovernError::Refused { .. } => ErrorKind::Governance,
        GovernError::Unavailable { .. } => ErrorKind::Unavailable,
    }
}

const fn model_kind(error: &LlmError) -> ErrorKind {
    match error {
        LlmError::ContextOverflow { .. } => ErrorKind::ContextOverflow,
        LlmError::Aborted => ErrorKind::Aborted,
        LlmError::Transport(_) => ErrorKind::Unavailable,
        _ => ErrorKind::Model,
    }
}
