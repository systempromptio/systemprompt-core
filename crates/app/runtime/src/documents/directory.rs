//! The user-backed [`SubjectDirectory`] the document store resolves grant
//! subjects through.
//!
//! A `user` subject names itself when the user is active, a `role` subject
//! names the active users holding the role, and a subject dimension names
//! nobody: dimension values have no reverse index, so events addressed by
//! dimension reach only users found another way (presence).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_files::documents::{SubjectDirectory, SubjectDirectoryError};
use systemprompt_identifiers::UserId;
use systemprompt_users::{User, UserError, UserRepository};

use super::subjects::{ROLE_SUBJECT_KIND, USER_SUBJECT_KIND};

/// The [`SubjectDirectory`] over the user repository.
#[derive(Debug, Clone)]
pub struct UserSubjectDirectory {
    users: Arc<UserRepository>,
}

impl UserSubjectDirectory {
    #[must_use]
    pub const fn new(users: Arc<UserRepository>) -> Self {
        Self { users }
    }
}

fn lookup_failed(error: UserError) -> SubjectDirectoryError {
    SubjectDirectoryError(Box::new(error))
}

#[async_trait]
impl SubjectDirectory for UserSubjectDirectory {
    async fn roles_of(&self, user: &UserId) -> Result<Option<Vec<String>>, SubjectDirectoryError> {
        let found = self.users.find_by_id(user).await.map_err(lookup_failed)?;
        Ok(found.filter(User::is_active).map(|u| u.roles))
    }

    async fn members(
        &self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<UserId>, SubjectDirectoryError> {
        match kind {
            USER_SUBJECT_KIND => {
                let id = UserId::new(value);
                let found = self.users.find_by_id(&id).await.map_err(lookup_failed)?;
                Ok(found
                    .filter(User::is_active)
                    .map(|u| u.id)
                    .into_iter()
                    .collect())
            },
            ROLE_SUBJECT_KIND => self
                .users
                .list_active_ids_with_role(value, i64::try_from(limit).unwrap_or(i64::MAX))
                .await
                .map_err(lookup_failed),
            _ => Ok(Vec::new()),
        }
    }
}
