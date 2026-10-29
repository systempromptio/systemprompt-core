//! [`AgentAccessPolicy`]: the seam an extension decides agent caps through.
//!
//! Core holds the mechanics; the policy is the extension's. An extension
//! registers one with
//! [`register_agent_access_policy!`](crate::register_agent_access_policy);
//! with none registered, [`DefaultAgentAccessPolicy`] answers `edit` on every
//! document and allows no [`AgentCapability`], which is today's behaviour
//! plus the hard rules. When several are registered the one whose name sorts
//! first wins and the rest are logged as ignored, so the choice does not
//! depend on link order.
//!
//! A registration builds its policy from the database pool, so a policy can
//! read its own tables. `cap` and `allows` are async and infallible: a policy
//! that cannot decide fails closed itself (`AgentCap::None`, `false`) and
//! logs why.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};
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
    pub created_by_agent: Option<String>,
    pub on_behalf_of: UserId,
}

/// The seam an extension sets agent caps through. `#[async_trait]` keeps it
/// dyn-compatible: the access service holds it as `Arc<dyn AgentAccessPolicy>`
/// built from its inventory registration.
///
/// `cap` is the most an agent may do on a target (an owner override only
/// lowers it); `allows` grants one [`AgentCapability`] (denied by default);
/// `on_move` runs before a document or folder move commits and may refuse it;
/// `on_copy` runs after every copy. A policy that cannot decide fails closed
/// itself.
#[async_trait]
pub trait AgentAccessPolicy: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    async fn cap(&self, _doc: &DocumentContext, _agent: &AgentActor) -> AgentCap {
        AgentCap::Edit
    }

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
}

/// One registered policy, submitted through
/// [`register_agent_access_policy!`](crate::register_agent_access_policy).
#[derive(Debug, Clone, Copy)]
pub struct AgentAccessPolicyRegistration {
    pub name: &'static str,
    pub build: fn(&DbPool) -> Arc<dyn AgentAccessPolicy>,
}

inventory::collect!(AgentAccessPolicyRegistration);

#[macro_export]
macro_rules! register_agent_access_policy {
    ($name:expr, $build:expr $(,)?) => {
        $crate::__inventory::submit! {
            $crate::documents::agents::AgentAccessPolicyRegistration {
                name: $name,
                build: $build,
            }
        }
    };
}

#[must_use]
pub fn registered_policy(db: &DbPool) -> Arc<dyn AgentAccessPolicy> {
    let mut registered: Vec<&AgentAccessPolicyRegistration> =
        inventory::iter::<AgentAccessPolicyRegistration>().collect();
    registered.sort_by_key(|registration| registration.name);
    let Some((chosen, ignored)) = registered.split_first() else {
        return Arc::new(DefaultAgentAccessPolicy);
    };
    for other in ignored {
        tracing::warn!(
            chosen = chosen.name,
            ignored = other.name,
            "More than one agent access policy is registered; only the first by name applies"
        );
    }
    (chosen.build)(db)
}
