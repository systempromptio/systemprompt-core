//! Applying a folder's move, rename and agent edit policy as one unit: the
//! three writes share one tree-locked transaction, so a folder never lands
//! half-updated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::documents::AgentEditPolicy;

use super::folder_locks::begin_tree_locked;
use super::folder_move::{FolderMoveOutcome, FolderRelocation, relocate_in, rename_in};
use super::folders::{DocumentFolderRepository, set_agent_edit_policy_in};
use super::rows::RepoResult;

#[derive(Debug, Clone, Copy)]
pub struct FolderUpdate<'a> {
    pub folder: &'a DocumentFolderId,
    pub relocation: Option<FolderRelocation<'a>>,
    pub name: Option<&'a str>,
    pub agent_edit_policy: Option<AgentEditPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderUpdateOutcome {
    Updated,
    Missing,
    Refused,
    DestinationChanged,
}

impl DocumentFolderRepository {
    pub async fn update(&self, update: FolderUpdate<'_>) -> RepoResult<FolderUpdateOutcome> {
        let mut touched = vec![update.folder.as_str()];
        touched.extend(
            update
                .relocation
                .and_then(|relocation| relocation.parent)
                .map(DocumentFolderId::as_str),
        );
        let mut tx = begin_tree_locked(&self.pools.write, &touched).await?;
        if let Some(relocation) = update.relocation {
            match relocate_in(&mut tx, relocation).await? {
                FolderMoveOutcome::Moved { .. } => {},
                FolderMoveOutcome::Refused => {
                    tx.rollback().await?;
                    return Ok(FolderUpdateOutcome::Refused);
                },
                FolderMoveOutcome::DestinationChanged => {
                    tx.rollback().await?;
                    return Ok(FolderUpdateOutcome::DestinationChanged);
                },
            }
        }
        if let Some(name) = update.name
            && !rename_in(&mut tx, update.folder, name).await?
        {
            tx.rollback().await?;
            return Ok(FolderUpdateOutcome::Missing);
        }
        if let Some(policy) = update.agent_edit_policy
            && !set_agent_edit_policy_in(&mut *tx, update.folder, policy).await?
        {
            tx.rollback().await?;
            return Ok(FolderUpdateOutcome::Missing);
        }
        tx.commit().await?;
        Ok(FolderUpdateOutcome::Updated)
    }
}
