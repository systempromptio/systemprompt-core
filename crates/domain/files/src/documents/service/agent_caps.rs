//! Reading and setting the owner's agent cap override on a document or a
//! folder.
//!
//! Reading needs `view`; setting or clearing needs `own`, which an agent
//! never holds for this purpose (`own` actions are the `administer`
//! capability), so an agent cannot loosen its own cap. An override only
//! tightens: one looser than the policy's cap for the target is refused, and
//! the effective cap is always the stricter of the two. The cap reported for
//! a person is the one a generic attended agent named `agent` would act
//! under, or the agent they name with [`DocumentService::agent_cap_as`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::DocumentService;
use crate::documents::access::{AccessTarget, Granted};
use crate::documents::agents::{AgentActor, AgentCap, AgentRefusal, EffectiveCap, current_actor};
use crate::documents::error::DocumentsError;

const GENERIC_AGENT: &str = "agent";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentCapTarget {
    Document(DocumentId),
    Folder(DocumentFolderId),
}

impl AgentCapTarget {
    fn access_target(&self) -> AccessTarget {
        match self {
            Self::Document(id) => AccessTarget::Document(id.clone()),
            Self::Folder(id) => AccessTarget::Folder(Some(id.clone())),
        }
    }
}

/// The override set on the target itself, and the cap agents act under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCapView {
    pub target: AgentCapTarget,
    pub own_override: Option<AgentCap>,
    pub effective: EffectiveCap,
}

fn reported_agent() -> AgentActor {
    current_actor().as_agent().cloned().unwrap_or(AgentActor {
        name: GENERIC_AGENT.to_owned(),
        attended: true,
    })
}

impl DocumentService {
    pub async fn agent_cap(
        &self,
        who: &AuthenticatedUser,
        target: AgentCapTarget,
        trace: &TraceId,
    ) -> Result<AgentCapView, DocumentsError> {
        self.agent_cap_as(who, target, reported_agent(), trace)
            .await
    }

    /// The cap `agent` would act under on `target`, for a person checking
    /// what a named agent may do there.
    pub async fn agent_cap_as(
        &self,
        who: &AuthenticatedUser,
        target: AgentCapTarget,
        agent: AgentActor,
        trace: &TraceId,
    ) -> Result<AgentCapView, DocumentsError> {
        let granted = self
            .access
            .authorize(who, target.access_target(), DocumentAction::View, trace)
            .await?;
        self.cap_view(who, target, &granted, &agent).await
    }

    pub async fn set_agent_cap(
        &self,
        who: &AuthenticatedUser,
        target: AgentCapTarget,
        cap: Option<AgentCap>,
        trace: &TraceId,
    ) -> Result<AgentCapView, DocumentsError> {
        let granted = self
            .access
            .authorize(who, target.access_target(), DocumentAction::Own, trace)
            .await?;
        if let Some(requested) = cap {
            let policy_cap = self
                .access
                .agent_cap_for(who, &granted, &reported_agent())
                .await?
                .policy_cap;
            if requested > policy_cap {
                return Err(AgentRefusal::OverrideLooser {
                    requested,
                    policy_cap,
                }
                .into());
            }
        }
        let updated = match &target {
            AgentCapTarget::Document(id) => {
                self.repos.documents.set_document_agent_cap(id, cap).await?
            },
            AgentCapTarget::Folder(id) => {
                self.repos.documents.set_folder_agent_cap(id, cap).await?
            },
        };
        if !updated {
            return Err(match &target {
                AgentCapTarget::Document(id) => DocumentsError::not_found("document", id),
                AgentCapTarget::Folder(id) => DocumentsError::not_found("folder", id),
            });
        }
        tracing::info!(
            user_id = %who.id,
            cap_target = ?target,
            agent_cap = cap.map(AgentCap::as_str),
            trace_id = trace.as_str(),
            "Agent cap override set"
        );
        self.cap_view(who, target, &granted, &reported_agent())
            .await
    }

    async fn cap_view(
        &self,
        who: &AuthenticatedUser,
        target: AgentCapTarget,
        granted: &Granted,
        agent: &AgentActor,
    ) -> Result<AgentCapView, DocumentsError> {
        let effective = self.access.agent_cap_for(who, granted, agent).await?;
        let (document, folders) = match &target {
            AgentCapTarget::Document(id) => (id.clone(), Vec::new()),
            AgentCapTarget::Folder(id) => {
                (DocumentId::from_uuid(uuid::Uuid::nil()), vec![id.clone()])
            },
        };
        let own_override = self
            .repos
            .documents
            .agent_facts(&document, &folders)
            .await?
            .overrides
            .first()
            .map(crate::documents::repository::CapOverride::cap);
        Ok(AgentCapView {
            target,
            own_override,
            effective,
        })
    }
}
