//! [`AgentAccessPolicy`]: the seam an extension decides agent caps through.
//!
//! Core holds the mechanics; the policy is the extension's. An extension
//! contributes one to [`AgentAccessSlot`](crate::contract::AgentAccessSlot);
//! with none contributed, [`DefaultAgentAccessPolicy`] answers `edit` on every
//! document and allows no [`AgentCapability`], plus the hard rules. The slot
//! is single-valued: two contributed policies refuse the build.
//!
//! A policy is built by a factory over the resolved state, so a policy can
//! read its own tables. `cap` and `allows` are async and infallible: a policy
//! that cannot decide fails closed itself (`AgentCap::None`, `false`) and
//! logs why.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::{AgentName, DocumentFolderId, DocumentId, UserId};
use systemprompt_models::documents::DocumentFormat;

use super::actor::AgentActor;
use super::cap::{AgentCap, AgentCapability};

/// What a policy knows about the target: a document, or a folder when
/// `document_id` is `None`.
///
/// `folders` is nearest-first: `folders[0]` is the document's own folder, or
/// for a folder target the folder itself, followed by its parent and so on
/// up to the top-level folder; it is empty for a document or target at the
/// root. `created_by_agent` names the agent that created the document (the
/// name its MCP calls carried), `None` for a document a person created or
/// for a folder target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentContext {
    pub document_id: Option<DocumentId>,
    pub folders: Vec<DocumentFolderId>,
    pub owner: Option<UserId>,
    pub name: Option<String>,
    pub format: Option<DocumentFormat>,
    pub created_by_agent: Option<AgentName>,
    pub on_behalf_of: UserId,
}

/// The seam an extension sets agent caps through. `#[async_trait]` keeps it
/// dyn-compatible: the access service holds it as `Arc<dyn AgentAccessPolicy>`
/// built by [`AgentAccessSlot`](crate::contract::AgentAccessSlot).
///
/// `cap` is required, so no policy grants a cap by omission; it is the most an
/// agent may do on a target (an owner override only lowers it); `allows` grants
/// one [`AgentCapability`] (denied by default); `on_move` runs before a
/// document or folder move commits and may refuse it; `on_copy` runs after
/// every copy. A policy that cannot decide fails closed itself.
#[async_trait]
pub trait AgentAccessPolicy: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    async fn cap(&self, doc: &DocumentContext, agent: &AgentActor) -> AgentCap;

    async fn allows(
        &self,
        _capability: AgentCapability,
        _doc: &DocumentContext,
        _agent: &AgentActor,
    ) -> bool {
        false
    }

    async fn on_move(
        &self,
        _doc: &DocumentContext,
        _to: &[DocumentFolderId],
        _mover: &MoveActor,
    ) -> Result<(), MoveRefused> {
        Ok(())
    }

    async fn on_copy(&self, _source: &DocumentContext, _copy: &DocumentId) {}
}

/// Who is moving a document or folder.
///
/// The user, whether they are an administrator, the agent acting for them
/// (`None` for the person), and the reason the request gave (`None` when it
/// gave none; core's move routes take none today).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveActor {
    pub user: UserId,
    pub is_admin: bool,
    pub agent: Option<AgentActor>,
    pub reason: Option<String>,
}

/// Why a policy refused a move; its reason is shown to the mover.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct MoveRefused {
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultAgentAccessPolicy;

#[async_trait]
impl AgentAccessPolicy for DefaultAgentAccessPolicy {
    fn name(&self) -> &'static str {
        "default"
    }

    async fn cap(&self, _doc: &DocumentContext, _agent: &AgentActor) -> AgentCap {
        AgentCap::Edit
    }
}
