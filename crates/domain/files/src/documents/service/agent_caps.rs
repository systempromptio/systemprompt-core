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
//! A move or a copy pins the cap the target carried, keeping any stricter
//! override already set.
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
use crate::documents::models::{Document, DocumentRevision};

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
    /// For the agent asked about.
    pub effective: EffectiveCap,
    /// For the same agent with a person in its session, and without.
    pub attended: EffectiveCap,
    pub unattended: EffectiveCap,
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
        self.cap_view(who, target, &granted, &agent, trace).await
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
        self.cap_view(who, target, &granted, &reported_agent(), trace)
            .await
    }

    /// Sets `cap` as the target's override, or keeps a stricter one already
    /// there: what a move or a copy carries.
    pub(super) async fn pin_agent_cap(
        &self,
        target: &AgentCapTarget,
        cap: AgentCap,
    ) -> Result<(), DocumentsError> {
        let (document, folders) = match target {
            AgentCapTarget::Document(id) => (id.clone(), Vec::new()),
            AgentCapTarget::Folder(id) => {
                (DocumentId::from_uuid(uuid::Uuid::nil()), vec![id.clone()])
            },
        };
        let existing = self
            .repos
            .documents
            .agent_facts(&document, &folders)
            .await?
            .overrides
            .first()
            .map(crate::documents::repository::CapOverride::cap);
        let pinned = existing.map_or(cap, |own| own.min(cap));
        match target {
            AgentCapTarget::Document(id) => {
                self.repos
                    .documents
                    .set_document_agent_cap(id, Some(pinned))
                    .await?
            },
            AgentCapTarget::Folder(id) => {
                self.repos
                    .documents
                    .set_folder_agent_cap(id, Some(pinned))
                    .await?
            },
        };
        Ok(())
    }

    /// A copy keeps its source's cap: `carried` is pinned on the new
    /// document, then the policy's `on_copy` sees it.
    pub(super) async fn carry_cap(
        &self,
        who: &AuthenticatedUser,
        source: &Document,
        copied: (Document, DocumentRevision),
        carried: Option<AgentCap>,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        if let Some(cap) = carried {
            self.pin_agent_cap(&AgentCapTarget::Document(copied.0.id.clone()), cap)
                .await?;
        }
        self.access.notify_copy(who, source, &copied.0.id).await?;
        Ok(copied)
    }

    async fn cap_view(
        &self,
        who: &AuthenticatedUser,
        target: AgentCapTarget,
        granted: &Granted,
        agent: &AgentActor,
        trace: &TraceId,
    ) -> Result<AgentCapView, DocumentsError> {
        let as_attended = |attended: bool| AgentActor {
            name: agent.name.clone(),
            attended,
        };
        let attended = self
            .access
            .agent_cap_for(who, granted, &as_attended(true))
            .await?;
        let unattended = self
            .access
            .agent_cap_for(who, granted, &as_attended(false))
            .await?;
        let effective = if agent.attended {
            attended.clone()
        } else {
            unattended.clone()
        };
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
            effective: self.access.redact_source(who, effective, trace).await,
            attended: self.access.redact_source(who, attended, trace).await,
            unattended: self.access.redact_source(who, unattended, trace).await,
        })
    }
}
