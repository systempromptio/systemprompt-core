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

pub(super) fn granted_ceiling(level: AccessLevel) -> AccessLevel {
    level.min(AccessLevel::Share)
}

impl DocumentAccessService {
    pub(super) async fn resolve(
        &self,
        who: &AuthenticatedUser,
        target: &AccessTarget,
        action: DocumentAction,
        pairs: &SubjectPairs,
    ) -> Result<Resolved, DocumentsError> {
        match target {
            AccessTarget::Document(id) | AccessTarget::Trashed(id) => {
                let trashed = matches!(target, AccessTarget::Trashed(_));
                let document = self
                    .documents
                    .find_including_trashed(id)
                    .await?
                    .ok_or_else(|| DocumentsError::not_found("document", id))?;
                let mut chain = Vec::new();
                if let Some(folder) = &document.folder_id {
                    chain.push(folder.clone());
                    chain.extend(self.folders.ancestors(folder).await?);
                }
                let level = if document.owner_user_id == who.id {
                    Some(AccessLevel::Own)
                } else {
                    self.grants
                        .effective_level(id, &chain, pairs)
                        .await?
                        .map(granted_ceiling)
                };
                match (document.trashed_at.is_some(), trashed) {
                    (true, false) if level.is_some() => {
                        return Err(DocumentsError::Trashed {
                            document_id: id.clone(),
                        });
                    },
                    (true, false) => return Err(DocumentsError::not_found("document", id)),
                    (false, true) => return Err(DocumentsError::not_found("trashed document", id)),
                    _ => {},
                }
                Ok(Resolved {
                    level,
                    nearest_folder: document.folder_id.clone(),
                    document: Some(document),
                    folder: None,
                    chain,
                })
            },
            AccessTarget::Folder(Some(id)) => {
                let folder = self
                    .folders
                    .find(id)
                    .await?
                    .filter(|f| f.trashed_at.is_none())
                    .ok_or_else(|| DocumentsError::not_found("folder", id))?;
                let mut chain = vec![folder.id.clone()];
                chain.extend(folder.ancestor_ids.iter().cloned());
                let level = if folder.owner_user_id == who.id {
                    Some(AccessLevel::Own)
                } else {
                    self.grants
                        .effective_level(&no_document(), &chain, pairs)
                        .await?
                        .map(granted_ceiling)
                };
                Ok(Resolved {
                    level,
                    nearest_folder: Some(folder.id.clone()),
                    document: None,
                    folder: Some(folder),
                    chain,
                })
            },
            AccessTarget::Folder(None) => Ok(Resolved {
                level: matches!(action, DocumentAction::Create | DocumentAction::View)
                    .then(|| action.required_level()),
                document: None,
                folder: None,
                nearest_folder: None,
                chain: Vec::new(),
            }),
        }
    }
}
