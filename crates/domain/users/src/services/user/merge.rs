//! Account merge and anonymous promotion across every owning domain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

use super::UserService;
use crate::error::{Result, UserError};
use crate::models::UserRole;
use crate::repository::MergeResult;

impl UserService {
    pub async fn merge_users(&self, source_id: &UserId, target_id: &UserId) -> Result<MergeResult> {
        if self.owner_reassignments.is_empty() {
            return Err(UserError::MergeUnavailable);
        }
        if source_id == target_id {
            return Err(UserError::Validation(
                "cannot merge a user into itself".to_owned(),
            ));
        }
        for id in [source_id, target_id] {
            if self.repository.find_by_id(id).await?.is_none() {
                return Err(UserError::NotFound(id.clone()));
            }
        }

        self.repository
            .merge(source_id, target_id, &self.owner_reassignments)
            .await
    }

    pub async fn promote_anonymous(
        &self,
        source_id: &UserId,
        target_id: &UserId,
    ) -> Result<MergeResult> {
        if source_id == target_id {
            return Err(UserError::Validation(
                "cannot promote a user onto itself".to_owned(),
            ));
        }
        let source = self
            .repository
            .find_by_id(source_id)
            .await?
            .ok_or_else(|| UserError::NotFound(source_id.clone()))?;
        if !source.has_role(UserRole::Anonymous) {
            return Err(UserError::Validation(format!(
                "user {} is not anonymous; use an explicit admin merge instead",
                source_id
            )));
        }
        self.merge_users(source_id, target_id).await
    }
}
