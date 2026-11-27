//! Error bodies: the platform's envelope on bridge and Drive routes, problem
//! documents on consumer routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The platform's error envelope (`code`, `message`, `error_key`, `timestamp`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error_key: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// An RFC 7807 problem document, as the consumer routes answer errors.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProblemDocument {
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<u16>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ProblemDocument {
    #[must_use]
    pub fn summary(&self) -> Option<&str> {
        self.detail.as_deref().or(self.message.as_deref())
    }
}
