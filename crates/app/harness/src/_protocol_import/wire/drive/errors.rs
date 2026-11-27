//! The Drive's error body and the head a conflict reports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The head the instance reports on a `409`, at the top level or under
/// `detail`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveConflictDetail {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub head_revision_id: Option<String>,
    #[serde(default)]
    pub head_content_sha256: Option<String>,
    #[serde(default)]
    pub base_revision_id: Option<String>,
}

/// The Drive's error body: the platform envelope plus the conflict detail.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveErrorBody {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error_key: Option<String>,
    #[serde(default)]
    pub head_revision_id: Option<String>,
    #[serde(default)]
    pub head_content_sha256: Option<String>,
    #[serde(default)]
    pub base_revision_id: Option<String>,
    #[serde(default)]
    pub detail: Option<DriveConflictDetail>,
}

impl DriveErrorBody {
    #[must_use]
    pub fn head_revision_id(&self) -> Option<&str> {
        self.head_revision_id.as_deref().or_else(|| {
            self.detail
                .as_ref()
                .and_then(|d| d.head_revision_id.as_deref())
        })
    }

    #[must_use]
    pub fn is_agent_refusal(&self) -> bool {
        self.error_key
            .as_deref()
            .is_some_and(|key| key.starts_with("agent_"))
    }
}
