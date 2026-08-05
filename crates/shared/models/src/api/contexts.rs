//! `ContextKind` classification for conversation contexts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "snake_case"))]
#[serde(rename_all = "snake_case")]
pub enum ContextKind {
    User,
    CliSession,
    Session,
    Evaluation,
    McpValidation,
    CliProbe,
    Derived,
    Legacy,
}

impl ContextKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::CliSession => "cli_session",
            Self::Session => "session",
            Self::Evaluation => "evaluation",
            Self::McpValidation => "mcp_validation",
            Self::CliProbe => "cli_probe",
            Self::Derived => "derived",
            Self::Legacy => "legacy",
        }
    }
}

impl fmt::Display for ContextKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown context kind: {0}")]
pub struct ParseContextKindError(String);

impl FromStr for ContextKind {
    type Err = ParseContextKindError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "cli_session" => Ok(Self::CliSession),
            "session" => Ok(Self::Session),
            "evaluation" => Ok(Self::Evaluation),
            "mcp_validation" => Ok(Self::McpValidation),
            "cli_probe" => Ok(Self::CliProbe),
            "derived" => Ok(Self::Derived),
            "legacy" => Ok(Self::Legacy),
            other => Err(ParseContextKindError(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub kind: ContextKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContextWithStats {
    pub context_id: ContextId,
    pub user_id: UserId,
    pub name: String,
    pub kind: ContextKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub task_count: i64,
    pub message_count: i64,
    pub last_message_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContextRequest {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateContextRequest {
    pub name: String,
}
