//! The typed failures a gateway dispatch can end in, downcast by the HTTP
//! layer to choose a status and envelope.
//!
//! [`DispatchError`] separates a failure that happened before the audit row
//! was opened from one already recorded against it, so the route handler
//! knows whether it still owes an audit write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error(transparent)]
    PreAudit(anyhow::Error),
    #[error(transparent)]
    Recorded(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PolicyDenied(pub String);

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct QuotaExceeded {
    pub message: String,
    pub retry_after_seconds: i32,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GuardForbidden {
    pub message: String,
}

/// A denial from the typed four-stage governance chain — the same engine and
/// the same operator-configured policies that govern MCP tool calls.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GovernanceDenied {
    pub policy: String,
    pub message: String,
}

/// A secret-scan denial the gateway could not repair; `locations` names the
/// provider-payload fields the client must fix before retrying.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct PromptRepairRequired {
    pub message: String,
    pub locations: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct SafetyBlocked {
    pub category: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GuardUnavailable {
    pub message: String,
    pub retry_after_seconds: i32,
}
