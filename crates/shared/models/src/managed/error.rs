//! Failures raised while validating or verifying a revision closure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ResourceRevisionId;

#[derive(Debug, thiserror::Error)]
pub enum RevisionBundleError {
    #[error("Invalid managed resource: {0}")]
    Invalid(String),
    #[error("Managed resource integrity check failed")]
    Integrity,
    #[error("Revision {0} is not part of the bundle")]
    MissingRevision(ResourceRevisionId),
    #[error("Managed resource serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub(super) fn invalid(message: &str) -> RevisionBundleError {
    RevisionBundleError::Invalid(message.to_owned())
}
