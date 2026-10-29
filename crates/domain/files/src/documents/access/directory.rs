//! [`SubjectDirectory`]: the users behind grant subjects.
//!
//! The document store keeps grants by subject pair and never reads the user
//! table itself; the composition root supplies a directory backed by the
//! user repository. It answers which roles an active user holds (to evaluate
//! another user's level when filtering event recipients), which active users
//! a subject names (to address `document.shared` and to clear presence when
//! a grant goes away), which active users match a name prefix and which
//! roles exist (to search and validate grant subjects).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

/// One subject a search found: the value a grant names, the name shown for
/// it and, for a user, their email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectMatch {
    pub value: String,
    pub display: String,
    pub detail: Option<String>,
}

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

    async fn roles_of_many(
        &self,
        users: &[UserId],
    ) -> Result<Vec<Option<Vec<String>>>, SubjectDirectoryError>;

    async fn members(
        &self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<UserId>, SubjectDirectoryError>;

    async fn search_users(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SubjectMatch>, SubjectDirectoryError>;

    async fn roles(&self) -> Result<Vec<String>, SubjectDirectoryError>;

    async fn is_person(&self, user: &UserId) -> Result<bool, SubjectDirectoryError> {
        Ok(self.roles_of(user).await?.is_some())
    }
}
