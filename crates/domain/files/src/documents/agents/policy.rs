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

use super::actor::{AgentActor, DocumentActor};
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

#[async_trait]
pub trait AgentAccessPolicy: Send + Sync + 'static {
    /// A short stable name, shown in `documents_access` as the cap's source.
    fn name(&self) -> &'static str;

    /// The most `agent` may do on the target. The owner's override can only
    /// lower it.
    async fn cap(&self, doc: &DocumentContext, agent: &AgentActor) -> AgentCap {
        let _ = (doc, agent);
        AgentCap::Edit
    }

    /// Whether `agent` may take `capability` on the target. Denied by
    /// default.
    async fn allows(
        &self,
        capability: AgentCapability,
        doc: &DocumentContext,
        agent: &AgentActor,
    ) -> bool {
        let _ = (capability, doc, agent);
        false
    }

    /// Called before a document or folder moves, by a person or an agent.
    /// Core already pins the source's cap when the destination would loosen
    /// it; a policy may refuse the move or pin a stricter cap.
    async fn on_move(&self, moving: &MoveContext<'_>) -> MoveDecision {
        let _ = moving;
        MoveDecision::Allow
    }
}

/// A move about to happen: the target as it stands (`doc.folders` is its
/// chain at the source), the destination's nearest-first chain (empty for
/// the root) and who is moving it.
#[derive(Debug, Clone, Copy)]
pub struct MoveContext<'a> {
    pub doc: &'a DocumentContext,
    pub to: &'a [DocumentFolderId],
    pub actor: &'a DocumentActor,
}

/// What a policy says about a move. `Pin` sets an override on the moved
/// document or folder (it only ever tightens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveDecision {
    Allow,
    Refuse { reason: String },
    Pin(AgentCap),
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

/// Registers an [`AgentAccessPolicy`]: `register_agent_access_policy!("name",
/// build)` with `fn build(db: &DbPool) -> Arc<dyn AgentAccessPolicy>`. Through
/// the facade it is `systemprompt::register_agent_access_policy!`.
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

/// The policy in force, built over `db`: the registered one, or the
/// default.
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
