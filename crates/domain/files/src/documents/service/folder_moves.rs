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
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentFolder;
use crate::documents::repository::{FolderMoveOutcome, FolderRelocation};

impl DocumentService {
    pub async fn move_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        parent: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
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
        let relocation = FolderRelocation {
            folder,
            parent,
            parent_chain: &parent_chain,
            mover: &who.id,
        };
        match self.repos.folders.move_to(relocation).await? {
            FolderMoveOutcome::Moved { .. } => {},
            FolderMoveOutcome::Refused => {
                return Err(DocumentsError::InvalidMove {
                    folder_id: folder.clone(),
                });
            },
            FolderMoveOutcome::DestinationChanged => {
                return Err(DocumentsError::SubtreeChanged {
                    folder_id: folder.clone(),
                });
            },
        }
        if let Some(cap) = pin {
            self.pin_agent_cap(&AgentCapTarget::Folder(folder.clone()), cap)
                .await?;
        }
        self.load_folder(folder).await
    }
}
