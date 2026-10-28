//! Caps that must survive a move or a copy, and the policy's say over a
//! move.
//!
//! A cap is judged for an unnamed, unattended agent, the strictest case a
//! name-blind override can stand for. Before a document or folder moves the
//! policy's [`AgentAccessPolicy::on_move`] is asked and may refuse or pin;
//! when the destination would loosen the cap, the source's cap is pinned on
//! the target as an override, so a move never launders a cap. A copy carries
//! its source's cap the same way, and an agent copying out of a capped
//! document needs `copy_out`.
//!
//! [`AgentAccessPolicy::on_move`]: crate::documents::agents::AgentAccessPolicy::on_move
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::auth::AuthenticatedUser;

use super::agent::{document_context, folder_context};
use super::service::DocumentAccessService;
use crate::documents::agents::{
    AgentActor, AgentCap, AgentCapability, AgentRefusal, DocumentContext, MoveContext,
    MoveDecision, UNNAMED_AGENT, current_actor,
};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder};

/// What is moving.
#[derive(Debug, Clone, Copy)]
pub enum MoveTarget<'a> {
    Document(&'a Document),
    Folder(&'a DocumentFolder),
}

fn parent_of(target: MoveTarget<'_>) -> Option<DocumentFolderId> {
    match target {
        MoveTarget::Document(document) => document.folder_id.clone(),
        MoveTarget::Folder(folder) => folder.parent_id.clone(),
    }
}

fn strictest_agent() -> AgentActor {
    AgentActor {
        name: UNNAMED_AGENT.to_owned(),
        attended: false,
    }
}

impl DocumentAccessService {
    /// The nearest-first chain of `folder`: the folder itself, then each
    /// folder above it; empty for the root.
    pub async fn chain_from(
        &self,
        folder: Option<&DocumentFolderId>,
    ) -> Result<Vec<DocumentFolderId>, DocumentsError> {
        let Some(folder) = folder else {
            return Ok(Vec::new());
        };
        let mut chain = vec![folder.clone()];
        chain.extend(self.folders.ancestors(folder).await?.into_iter().rev());
        Ok(chain)
    }

    fn context_at(
        who: &AuthenticatedUser,
        target: MoveTarget<'_>,
        parent_chain: Vec<DocumentFolderId>,
    ) -> DocumentContext {
        match target {
            MoveTarget::Document(document) => document_context(who, document, parent_chain),
            MoveTarget::Folder(folder) => {
                let mut chain = vec![folder.id.clone()];
                chain.extend(parent_chain);
                folder_context(who, Some(folder), chain)
            },
        }
    }

    /// The cap an unnamed, unattended agent holds on `target` where it
    /// stands.
    pub async fn strictest_cap(
        &self,
        who: &AuthenticatedUser,
        target: MoveTarget<'_>,
    ) -> Result<AgentCap, DocumentsError> {
        let chain = self.chain_from(parent_of(target).as_ref()).await?;
        let mut context = Self::context_at(who, target, chain);
        Ok(self
            .effective_cap(&mut context, &strictest_agent())
            .await?
            .cap)
    }

    /// Asks the policy about moving `target` under `to` and returns the cap
    /// to pin on it after the move, if any: the policy's pin, or the
    /// source's cap when the destination would loosen it. A refusal is
    /// `MoveRefused`.
    pub async fn check_move(
        &self,
        who: &AuthenticatedUser,
        target: MoveTarget<'_>,
        to: Option<&DocumentFolderId>,
    ) -> Result<Option<AgentCap>, DocumentsError> {
        let from_chain = self.chain_from(parent_of(target).as_ref()).await?;
        let mut source = Self::context_at(who, target, from_chain);
        let before = self
            .effective_cap(&mut source, &strictest_agent())
            .await?
            .cap;
        let to_chain = self.chain_from(to).await?;
        let actor = current_actor();
        let decision = self
            .agents
            .on_move(&MoveContext {
                doc: &source,
                to: &to_chain,
                actor: &actor,
            })
            .await;
        let mut pin = match decision {
            MoveDecision::Refuse { reason } => {
                return Err(AgentRefusal::MoveRefused { reason }.into());
            },
            MoveDecision::Pin(cap) => Some(cap),
            MoveDecision::Allow => None,
        };
        let mut landed = Self::context_at(who, target, to_chain);
        let after = self
            .effective_cap(&mut landed, &strictest_agent())
            .await?
            .cap;
        if after > before {
            pin = Some(pin.map_or(before, |cap| cap.min(before)));
        }
        Ok(pin)
    }

    /// Refuses an agent caller copying out of `source` when its cap is below
    /// `edit` and the policy does not allow `copy_out`; returns the cap the
    /// copy must carry, if any.
    pub async fn check_copy(
        &self,
        who: &AuthenticatedUser,
        source: &Document,
    ) -> Result<Option<AgentCap>, DocumentsError> {
        let carried = self
            .strictest_cap(who, MoveTarget::Document(source))
            .await?;
        if carried >= AgentCap::Edit {
            return Ok(None);
        }
        self.require_agent_capability(who, source, AgentCapability::CopyOut)
            .await?;
        Ok(Some(carried))
    }
}
