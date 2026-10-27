//! [`SubjectDirectory`]: the users behind grant subjects.
//!
//! The document store keeps grants by subject pair and never reads the user
//! table itself; the composition root supplies a directory backed by the
//! user repository. It answers two questions: which roles an active user
//! holds (to evaluate another user's level when filtering event recipients),
//! and which active users a subject names (to address `document.shared` and
//! to clear presence when a grant goes away).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

#[derive(Debug, thiserror::Error)]
#[error("subject directory lookup failed")]
pub struct SubjectDirectoryError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

/// Resolves grant subjects to users. A subject kind the directory cannot
/// enumerate (a dimension without a reverse index) answers an empty list,
/// never an error.
///
/// `#[async_trait]` keeps the trait object-safe: the service holds it as
/// `Arc<dyn SubjectDirectory>`.
#[async_trait]
pub trait SubjectDirectory: Send + Sync {
    async fn roles_of(&self, user: &UserId) -> Result<Option<Vec<String>>, SubjectDirectoryError>;

    async fn members(
        &self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<UserId>, SubjectDirectoryError>;
}
