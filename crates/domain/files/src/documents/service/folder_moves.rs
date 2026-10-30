//! Moving a folder: the caller owns the folder and may create under the
//! destination, the destination's ancestor chain is read before that check
//! and compared again under the tree lock, and the repository moves the
//! documents other users own in the subtree to their owners' roots in the
//! same transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::{AgentCapTarget, DocumentService};
use crate::documents::access::{AccessTarget, MoveTarget};
use crate::documents::agents::AgentCap;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentFolder;
use crate::documents::repository::{FolderMoveOutcome, FolderRelocation};

pub(super) struct PreparedMove {
    pub(super) parent_chain: Vec<DocumentFolderId>,
    pub(super) pin: Option<AgentCap>,
}

pub(super) fn move_refusal(
    outcome: FolderMoveOutcome,
    folder: &DocumentFolderId,
) -> Option<DocumentsError> {
    match outcome {
        FolderMoveOutcome::Moved { .. } => None,
        FolderMoveOutcome::Refused => Some(DocumentsError::InvalidMove {
            folder_id: folder.clone(),
        }),
        FolderMoveOutcome::DestinationChanged => Some(DocumentsError::SubtreeChanged {
            folder_id: folder.clone(),
        }),
    }
}

impl DocumentService {
    pub(super) async fn prepare_folder_move(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        parent: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> Result<PreparedMove, DocumentsError> {
        if parent == Some(folder) {
            return Err(DocumentsError::InvalidMove {
                folder_id: folder.clone(),
            });
        }
        self.access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                DocumentAction::Own,
                trace,
            )
            .await?;
        let parent_chain = match parent {
            Some(parent) => self.repos.folders.ancestors(parent).await?,
            None => Vec::new(),
        };
        self.access
            .authorize(
                who,
                AccessTarget::Folder(parent.cloned()),
                DocumentAction::Create,
                trace,
            )
            .await?;
        let moving = self.load_folder(folder).await?;
        let pin = self
            .access
            .check_move(who, MoveTarget::Folder(&moving), parent)
            .await?;
        Ok(PreparedMove { parent_chain, pin })
    }

    pub async fn move_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        parent: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
        let prepared = self.prepare_folder_move(who, folder, parent, trace).await?;
        let relocation = FolderRelocation {
            folder,
            parent,
            parent_chain: &prepared.parent_chain,
            mover: &who.id,
        };
        let outcome = self.repos.folders.move_to(relocation).await?;
        if let Some(refusal) = move_refusal(outcome, folder) {
            return Err(refusal);
        }
        if let Some(cap) = prepared.pin {
            self.pin_agent_cap(&AgentCapTarget::Folder(folder.clone()), cap)
                .await?;
        }
        self.load_folder(folder).await
    }
}
