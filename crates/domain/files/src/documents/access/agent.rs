//! The agent step of an access check: the cap an agent caller acts under
//! on the target, the level it leaves the caller, and the hard rules.
//!
//! For a person this step is a no-op. For an agent it runs between grant
//! resolution and the authz hook, so a capped agent is refused before the
//! hook is asked and an allowed read under a cap below `edit` is audited
//! with the trace id once the hook allows it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

use super::request::not_found;
use super::service::{AccessTarget, DocumentAccessService, Resolved, no_document};
use crate::documents::agents::{
    AgentActor, AgentCap, AgentCapability, AgentRefusal, CapSource, DocumentContext, EffectiveCap,
    current_actor,
};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder, SubjectPairs};
use crate::documents::repository::{AgentReadRecord, CapOverride};

/// An agent caller's cap on one target and what the policy was asked with.
#[derive(Debug, Clone)]
pub(super) struct AgentGate {
    agent: AgentActor,
    effective: EffectiveCap,
    allowed: Vec<AgentCapability>,
}

impl AgentGate {
    /// The level the agent holds for `action`, given the user's own level.
    pub(super) fn held(
        &self,
        resolved: &Resolved,
        action: DocumentAction,
    ) -> Result<Option<AccessLevel>, DocumentsError> {
        let Some(held) = resolved.level else {
            return Ok(None);
        };
        let cap = self.effective.cap;
        let Some(capped) = cap.cap_level(held) else {
            return Err(not_found(resolved));
        };
        if let Some(capability) = AgentCapability::of_action(action)
            && !self.allowed.contains(&capability)
        {
            return Err(AgentRefusal::HardRule {
                agent: self.agent.name.clone(),
                capability,
            }
            .into());
        }
        let needed = action.required_level();
        if capped >= needed || held < needed {
            return Ok(Some(capped));
        }
        let reason = self.effective.reason();
        let edits = matches!(action, DocumentAction::Edit | DocumentAction::Create);
        if cap == AgentCap::Suggest && edits {
            return Err(AgentRefusal::SuggestOnly {
                agent: self.agent.name.clone(),
                reason,
            }
            .into());
        }
        Err(AgentRefusal::CapExceeded {
            agent: self.agent.name.clone(),
            cap,
            action: action.to_string(),
            reason,
        }
        .into())
    }

    pub(super) fn into_cap(self) -> EffectiveCap {
        self.effective
    }
}

const CAPABILITIES: [AgentCapability; 9] = [
    AgentCapability::Share,
    AgentCapability::MentionGrant,
    AgentCapability::Trash,
    AgentCapability::Administer,
    AgentCapability::RestoreRevision,
    AgentCapability::ReplaceContent,
    AgentCapability::ReviewProposal,
    AgentCapability::CopyOut,
    AgentCapability::EditTemplate,
];

impl DocumentAccessService {
    /// The gate for the running call's actor on `resolved`; `None` for a
    /// person.
    pub(super) async fn agent_gate(
        &self,
        who: &AuthenticatedUser,
        resolved: &Resolved,
        pairs: &SubjectPairs,
    ) -> Result<Option<AgentGate>, DocumentsError> {
        let Some(agent) = current_actor().as_agent().cloned() else {
            return Ok(None);
        };
        let mut context = context_of(who, resolved);
        let effective = self.effective_cap(&mut context, &agent).await?;
        let effective = self.hide_unviewable(who, effective, pairs).await?;
        let mut allowed = Vec::new();
        for capability in CAPABILITIES {
            if self.agents.allows(capability, &context, &agent).await {
                allowed.push(capability);
            }
        }
        Ok(Some(AgentGate {
            agent,
            effective,
            allowed,
        }))
    }

    /// The stricter of the policy's cap and every override on the target
    /// and the folders above it. Fills `context.created_by_agent` from the
    /// store before the policy is asked.
    pub(super) async fn effective_cap(
        &self,
        context: &mut DocumentContext,
        agent: &AgentActor,
    ) -> Result<EffectiveCap, DocumentsError> {
        let document = context.document_id.clone().unwrap_or_else(no_document);
        let facts = self
            .documents
            .agent_facts(&document, &context.folders)
            .await?;
        context.created_by_agent = facts.created_by_agent;
        let policy_cap = self.agents.cap(context, agent).await;
        Ok(combine(policy_cap, self.agents.name(), facts.overrides))
    }

    /// `effective` with a folder override's source replaced by
    /// [`CapSource::InheritedOverride`] when the caller holds no grant on
    /// that folder, so no reason or refusal echoes a folder id they cannot
    /// view. Grants only: it never asks the hook or the agent gate, so it
    /// cannot recurse.
    pub(super) async fn hide_unviewable(
        &self,
        who: &AuthenticatedUser,
        mut effective: EffectiveCap,
        pairs: &SubjectPairs,
    ) -> Result<EffectiveCap, DocumentsError> {
        let CapSource::FolderOverride { folder_id } = &effective.source else {
            return Ok(effective);
        };
        let target = AccessTarget::Folder(Some(folder_id.clone()));
        let visible = match self
            .resolve(who, &target, DocumentAction::View, pairs)
            .await
        {
            Ok(resolved) => resolved.level.is_some(),
            Err(DocumentsError::NotFound { .. }) => false,
            Err(other) => return Err(other),
        };
        if !visible {
            effective.source = CapSource::InheritedOverride;
        }
        Ok(effective)
    }

    /// The nearest-first folder chain of `document`: its folder, then each
    /// folder above it.
    pub(super) async fn chain_of(
        &self,
        document: &Document,
    ) -> Result<Vec<DocumentFolderId>, DocumentsError> {
        let mut folders = Vec::new();
        if let Some(folder) = &document.folder_id {
            folders.push(folder.clone());
            folders.extend(self.folders.ancestors(folder).await?.into_iter().rev());
        }
        Ok(folders)
    }

    pub(super) async fn audit_agent_read(
        &self,
        who: &AuthenticatedUser,
        gate: &AgentGate,
        document: &Document,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        if gate.effective.cap >= AgentCap::Edit || action != DocumentAction::View {
            return Ok(());
        }
        let source = gate.effective.source.label();
        self.documents
            .insert_agent_read(AgentReadRecord {
                document_id: &document.id,
                user_id: &who.id,
                agent_name: &gate.agent.name,
                attended: gate.agent.attended,
                action: "view",
                cap: gate.effective.cap,
                cap_source: &source,
                trace_id: trace,
            })
            .await?;
        Ok(())
    }
}

/// The stricter of `policy_cap` and every override, and which set it.
pub(super) fn combine(
    policy_cap: AgentCap,
    policy: &'static str,
    overrides: Vec<CapOverride>,
) -> EffectiveCap {
    let mut effective = EffectiveCap {
        cap: policy_cap,
        policy_cap,
        source: CapSource::Policy { policy },
    };
    for found in overrides {
        if found.cap() < effective.cap {
            effective.cap = found.cap();
            effective.source = match found {
                CapOverride::Document(document_id, _) => {
                    CapSource::DocumentOverride { document_id }
                },
                CapOverride::Folder(folder_id, _) => CapSource::FolderOverride { folder_id },
            };
        }
    }
    effective
}

pub(super) fn document_context(
    who: &AuthenticatedUser,
    document: &Document,
    folders: Vec<DocumentFolderId>,
) -> DocumentContext {
    DocumentContext {
        document_id: Some(document.id.clone()),
        folders,
        owner: Some(document.owner_user_id.clone()),
        name: Some(document.name.clone()),
        format: Some(document.format),
        created_by_agent: None,
        on_behalf_of: who.id.clone(),
    }
}

pub(super) fn folder_context(
    who: &AuthenticatedUser,
    folder: Option<&DocumentFolder>,
    folders: Vec<DocumentFolderId>,
) -> DocumentContext {
    DocumentContext {
        document_id: None,
        folders,
        owner: folder.map(|f| f.owner_user_id.clone()),
        name: folder.map(|f| f.name.clone()),
        format: None,
        created_by_agent: None,
        on_behalf_of: who.id.clone(),
    }
}

fn context_of(who: &AuthenticatedUser, resolved: &Resolved) -> DocumentContext {
    match &resolved.document {
        Some(document) => document_context(who, document, resolved.chain.clone()),
        None => folder_context(who, resolved.folder.as_ref(), resolved.chain.clone()),
    }
}
