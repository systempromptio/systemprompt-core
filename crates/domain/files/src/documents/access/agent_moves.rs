//! Caps that must survive a move or a copy, and the policy's hooks on
//! both.
//!
//! A cap is judged as the stricter of its attended and unattended cases, for
//! the agent moving it (or an unnamed agent when a person moves it), so a
//! pin holds whichever way an agent next comes. Before a document or folder
//! moves the policy's [`AgentAccessPolicy::on_move`] is asked and may refuse
//! it; when the destination would loosen the cap, the source's cap is pinned on
//! the target as an override, so a move never launders a cap. A copy carries
//! its source's cap the same way, and an agent copying out of a capped
//! document needs `copy_out`; the policy's `on_copy` then sees the copy.
//!
//! [`AgentAccessPolicy::on_move`]: crate::documents::agents::AgentAccessPolicy::on_move
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;

use super::agent::{document_context, folder_context};
use super::service::DocumentAccessService;
use crate::documents::agents::{
    AgentActor, AgentCap, AgentCapability, AgentChannel, AgentRefusal, DocumentContext, MoveActor,
    UNNAMED_AGENT, current_actor,
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
        channel: AgentChannel::Direct,
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

    /// Asks the policy about moving `target` under `to` (its `on_move` may
    /// refuse, as `MoveRefused`, or pin its own override) and returns the
    /// cap core pins after the move: the source's, when the destination
    /// would loosen it.
    pub async fn check_move(
        &self,
        who: &AuthenticatedUser,
        target: MoveTarget<'_>,
        to: Option<&DocumentFolderId>,
    ) -> Result<Option<AgentCap>, DocumentsError> {
        let judge = current_actor()
            .as_agent()
            .cloned()
            .unwrap_or_else(strictest_agent);
        let from_chain = self.chain_from(parent_of(target).as_ref()).await?;
        let mut source = Self::context_at(who, target, from_chain);
        let before = self.strictest_of(&mut source, &judge).await?;
        let to_chain = self.chain_from(to).await?;
        let mover = MoveActor {
            user: who.id.clone(),
            is_admin: who.is_admin(),
            agent: current_actor().as_agent().cloned(),
            reason: None,
        };
        self.agents
            .on_move(&source, &to_chain, &mover)
            .await
            .map_err(|reason| AgentRefusal::MoveRefused { reason })?;
        let mut landed = Self::context_at(who, target, to_chain);
        let after = self.strictest_of(&mut landed, &judge).await?;
        Ok((after > before).then_some(before))
    }

    /// The stricter of the caps `agent` holds on `context` attended and
    /// unattended: what a pin must keep whichever way the agent next comes.
    async fn strictest_of(
        &self,
        context: &mut DocumentContext,
        agent: &AgentActor,
    ) -> Result<AgentCap, DocumentsError> {
        let mut cap = AgentCap::Edit;
        for attended in [true, false] {
            let case = AgentActor {
                attended,
                ..agent.clone()
            };
            cap = cap.min(self.effective_cap(context, &case).await?.cap);
        }
        Ok(cap)
    }

    /// Tells the policy a copy of `source` now exists as `copy`.
    pub async fn notify_copy(
        &self,
        who: &AuthenticatedUser,
        source: &Document,
        copy: &DocumentId,
    ) -> Result<(), DocumentsError> {
        let chain = self.chain_from(source.folder_id.as_ref()).await?;
        let mut context = document_context(who, source, chain);
        self.effective_cap(&mut context, &strictest_agent()).await?;
        self.agents.on_copy(&context, copy).await;
        Ok(())
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
