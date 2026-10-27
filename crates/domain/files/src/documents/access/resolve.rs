//! Grant resolution: the level a user holds on a target before the hook is
//! asked.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

use super::service::{AccessTarget, DocumentAccessService, Resolved, no_document};
use crate::documents::error::DocumentsError;
use crate::documents::models::SubjectPairs;

impl DocumentAccessService {
    pub(super) async fn resolve(
        &self,
        who: &AuthenticatedUser,
        target: &AccessTarget,
        action: DocumentAction,
        pairs: &SubjectPairs,
    ) -> Result<Resolved, DocumentsError> {
        match target {
            AccessTarget::Document(id) => {
                let document = self
                    .documents
                    .find(id)
                    .await?
                    .filter(|d| d.deleted_at.is_none())
                    .ok_or_else(|| DocumentsError::not_found("document", id))?;
                let mut chain = Vec::new();
                if let Some(folder) = &document.folder_id {
                    chain.push(folder.clone());
                    chain.extend(self.folders.ancestors(folder).await?);
                }
                let level = if document.owner_user_id == who.id {
                    Some(AccessLevel::Own)
                } else {
                    self.grants.effective_level(id, &chain, pairs).await?
                };
                Ok(Resolved {
                    level,
                    nearest_folder: document.folder_id.clone(),
                    document: Some(document),
                    folder: None,
                })
            },
            AccessTarget::Folder(Some(id)) => {
                let folder = self
                    .folders
                    .find(id)
                    .await?
                    .filter(|f| f.deleted_at.is_none())
                    .ok_or_else(|| DocumentsError::not_found("folder", id))?;
                let level = if folder.owner_user_id == who.id {
                    Some(AccessLevel::Own)
                } else {
                    let mut chain = vec![folder.id.clone()];
                    chain.extend(folder.ancestor_ids.iter().cloned());
                    self.grants
                        .effective_level(&no_document(), &chain, pairs)
                        .await?
                };
                Ok(Resolved {
                    level,
                    nearest_folder: Some(folder.id.clone()),
                    document: None,
                    folder: Some(folder),
                })
            },
            AccessTarget::Folder(None) => Ok(Resolved {
                level: matches!(action, DocumentAction::Create | DocumentAction::View)
                    .then(|| action.required_level()),
                document: None,
                folder: None,
                nearest_folder: None,
            }),
        }
    }
}
