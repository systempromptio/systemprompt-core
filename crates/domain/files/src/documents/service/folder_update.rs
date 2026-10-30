//! Updating a folder in one step: a move, a rename and an agent edit policy
//! are each authorized for the caller before any is applied, then written in
//! one transaction, so a refusal or failure leaves the folder as it was.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AgentEditPolicy, DocumentAction};

use super::folder_moves::PreparedMove;
use super::support::validate_name;
use super::{AgentCapTarget, DocumentService, FolderMove};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentFolder;
use crate::documents::repository::{FolderRelocation, FolderUpdate, FolderUpdateOutcome};

/// The changes to make to one folder; at least one must be present.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderUpdateRequest {
    pub name: Option<String>,
    pub parent: FolderMove,
    pub agent_edit_policy: Option<AgentEditPolicy>,
}

impl FolderUpdateRequest {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.name.is_none() && self.parent.is_keep() && self.agent_edit_policy.is_none()
    }
}

impl DocumentService {
    pub async fn update_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        request: FolderUpdateRequest,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
        let FolderUpdateRequest {
            name,
            parent,
            agent_edit_policy,
        } = request;
        if let Some(name) = &name {
            validate_name(name)?;
            self.authorize_folder(who, folder, DocumentAction::Edit, trace)
                .await?;
        }
        if agent_edit_policy.is_some() {
            self.authorize_folder(who, folder, DocumentAction::Share, trace)
                .await?;
        }
        let prepared = match parent {
            FolderMove::Keep => None,
            FolderMove::Root => Some(self.prepare_folder_move(who, folder, None, trace).await?),
            FolderMove::Into(ref target) => Some(
                self.prepare_folder_move(who, folder, Some(target), trace)
                    .await?,
            ),
        };
        let relocation = prepared
            .as_ref()
            .map(|prepared: &PreparedMove| FolderRelocation {
                folder,
                parent: parent.target(),
                parent_chain: &prepared.parent_chain,
                mover: &who.id,
            });
        let update = FolderUpdate {
            folder,
            relocation,
            name: name.as_deref(),
            agent_edit_policy,
        };
        match self.repos.folders.update(update).await? {
            FolderUpdateOutcome::Updated => {},
            FolderUpdateOutcome::Missing => {
                return Err(DocumentsError::not_found("folder", folder));
            },
            FolderUpdateOutcome::Refused => {
                return Err(DocumentsError::InvalidMove {
                    folder_id: folder.clone(),
                });
            },
            FolderUpdateOutcome::DestinationChanged => {
                return Err(DocumentsError::SubtreeChanged {
                    folder_id: folder.clone(),
                });
            },
        }
        if let Some(cap) = prepared.and_then(|prepared| prepared.pin) {
            self.pin_agent_cap(&AgentCapTarget::Folder(folder.clone()), cap)
                .await?;
        }
        self.load_folder(folder).await
    }

    async fn authorize_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                action,
                trace,
            )
            .await?;
        Ok(())
    }
}
