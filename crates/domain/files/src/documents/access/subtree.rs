//! A recursive folder delete, decided for the whole subtree at once.
//!
//! The root folder is authorized with the authz hook's `evaluate`, which
//! writes the one audit row the delete carries; every folder beneath it is
//! then authorized with the hook's `probe` over subject pairs read once.
//!
//! Deleting a document takes `own`, which no grant reaches, so the caller's
//! own live documents are trashed with the subtree and every document someone
//! else owns is re-homed to its owner's root: the folder owner reshapes their
//! tree, never another user's data. Only an agent caller is gated per trashed
//! document, by its cap and hard rules, without a hook call or audit row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

use super::agent::held_level;
use super::service::{AccessCheck, AccessTarget, DocumentAccessService, Resolved};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;

/// A subtree's live documents split by what a recursive delete does to them.
#[derive(Debug, Clone, Default)]
pub struct SubtreePartition {
    pub trash: Vec<Document>,
    pub rehome: Vec<Document>,
}

impl DocumentAccessService {
    pub async fn authorize_subtree_folders(
        &self,
        who: &AuthenticatedUser,
        root: &DocumentFolderId,
        members: &[DocumentFolderId],
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.authorize(
            who,
            AccessTarget::Folder(Some(root.clone())),
            DocumentAction::Delete,
            trace,
        )
        .await?;
        let pairs = self.subject_pairs(who).await?;
        for member in members.iter().filter(|member| *member != root) {
            let target = AccessTarget::Folder(Some(member.clone()));
            let check = AccessCheck {
                who,
                target: &target,
                action: DocumentAction::Delete,
                trace,
            };
            self.probe_with(check, &pairs).await?;
        }
        Ok(())
    }

    pub async fn partition_subtree_documents(
        &self,
        who: &AuthenticatedUser,
        documents: Vec<Document>,
    ) -> Result<SubtreePartition, DocumentsError> {
        let (trash, rehome): (Vec<Document>, Vec<Document>) = documents
            .into_iter()
            .partition(|document| document.owner_user_id == who.id);
        if self.actor().as_agent().is_some() {
            self.gate_agent_trash(who, &trash).await?;
        }
        Ok(SubtreePartition { trash, rehome })
    }

    async fn gate_agent_trash(
        &self,
        who: &AuthenticatedUser,
        documents: &[Document],
    ) -> Result<(), DocumentsError> {
        let pairs = self.subject_pairs(who).await?;
        let mut chains: HashMap<Option<DocumentFolderId>, Vec<DocumentFolderId>> = HashMap::new();
        for document in documents {
            let chain = if let Some(chain) = chains.get(&document.folder_id) {
                chain.clone()
            } else {
                let chain = self.chain_of(document).await?;
                chains.insert(document.folder_id.clone(), chain.clone());
                chain
            };
            let resolved = Resolved {
                level: Some(AccessLevel::Own),
                nearest_folder: document.folder_id.clone(),
                document: Some(document.clone()),
                folder: None,
                chain,
            };
            let gate = self.agent_gate(who, &resolved, &pairs).await?;
            held_level(gate.as_ref(), &resolved, DocumentAction::Delete)?;
        }
        Ok(())
    }
}
