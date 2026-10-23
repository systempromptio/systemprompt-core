//! Skill feedback contracts shared across ingestion, marketplace, evaluators
//! and clients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod inventory;
pub mod receipts;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bridge::host::{HostAppKind, HostKind, HostProduct};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EvaluatorClient {
    ClaudeCode,
    OpenCode,
    Codex,
    Hermes,
    ClaudeDesktop,
}

impl EvaluatorClient {
    #[must_use]
    pub fn accepts(self, host: HostKind) -> bool {
        Self::from(host) == self
    }
}

impl From<HostKind> for EvaluatorClient {
    fn from(host: HostKind) -> Self {
        match host.product() {
            HostProduct::Claude => match host.kind() {
                HostAppKind::DesktopApp => Self::ClaudeDesktop,
                HostAppKind::CliTool => Self::ClaudeCode,
            },
            HostProduct::Codex => Self::Codex,
            HostProduct::OpenCode => Self::OpenCode,
            HostProduct::Hermes => Self::Hermes,
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
pub struct ContentDigest(String);

impl ContentDigest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentDigest {
    type Error = FeedbackContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(FeedbackContractError::InvalidDigest);
        }
        Ok(Self(value))
    }
}

impl From<ContentDigest> for String {
    fn from(value: ContentDigest) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FeedbackContractError {
    #[error("Expected a lowercase SHA-256 digest")]
    InvalidDigest,
    #[error("Invalid relative path")]
    InvalidPath,
    #[error("Verification manifest is incomplete or inconsistent")]
    IncompleteManifest,
    #[error("Input exceeds contract bounds")]
    Bounds,
}

pub fn validate_relative_path(path: &str) -> Result<(), FeedbackContractError> {
    if path.is_empty()
        || path.len() > 4096
        || path.contains(['\\', ':', '\0'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(FeedbackContractError::InvalidPath);
    }
    Ok(())
}
