//! Skill feedback contracts shared across ingestion, marketplace, evaluators
//! and clients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod inventory;
pub mod receipts;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EvaluatorClient {
    ClaudeCode,
    #[serde(alias = "opencode")]
    OpenCode,
    Codex,
    Hermes,
    ClaudeDesktop,
}

impl EvaluatorClient {
    pub fn accepts_host_name(self, name: &str) -> bool {
        match self {
            Self::ClaudeCode => name == "claude-code",
            Self::ClaudeDesktop => name == "claude-desktop",
            Self::Codex => matches!(name, "codex" | "codex-cli"),
            Self::OpenCode => matches!(name, "opencode" | "open-code"),
            Self::Hermes => name == "hermes",
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
