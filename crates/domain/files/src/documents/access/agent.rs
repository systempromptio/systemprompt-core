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
use super::service::{DocumentAccessService, Granted, Resolved, no_document};
use crate::documents::agents::{
    AgentActor, AgentCap, AgentCapability, AgentRefusal, CapSource, DocumentContext, EffectiveCap,
    current_actor,
};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
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

impl DocumentAccessService {
    /// The gate for the running call's actor on `resolved`; `None` for a
    /// person.
    pub(super) async fn agent_gate(
        &self,
        who: &AuthenticatedUser,
        resolved: &Resolved,
    ) -> Result<Option<AgentGate>, DocumentsError> {
        let Some(agent) = current_actor().as_agent().cloned() else {
            return Ok(None);
        };
        let context = context_of(who, resolved);
        let effective = self.effective_cap(&context, &agent).await?;
        let allowed = [
            AgentCapability::Share,
            AgentCapability::MentionGrant,
            AgentCapability::Trash,
            AgentCapability::Administer,
            AgentCapability::RestoreRevision,
            AgentCapability::ReplaceContent,
            AgentCapability::EditTemplate,
        ]
        .into_iter()
        .filter(|capability| self.agents.allows(*capability, &context, &agent))
        .collect();
        Ok(Some(AgentGate {
            agent,
            effective,
            allowed,
        }))
    }

    /// The stricter of the policy's cap and every override on the target
    /// and the folders above it.
    pub(super) async fn effective_cap(
        &self,
        context: &DocumentContext,
        agent: &AgentActor,
    ) -> Result<EffectiveCap, DocumentsError> {
        let policy_cap = self.agents.cap(context, agent);
        let document = context.document_id.clone().unwrap_or_else(no_document);
        let overrides = self
            .documents
            .cap_overrides(&document, &context.folders)
            .await?;
        let mut effective = EffectiveCap {
            cap: policy_cap,
            policy_cap,
            source: CapSource::Policy {
                policy: self.agents.name(),
            },
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
        Ok(effective)
    }

    /// Refuses `capability` to an agent caller on `document` unless the
    /// policy allows it; a person always passes.
    pub async fn require_agent_capability(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        capability: AgentCapability,
    ) -> Result<(), DocumentsError> {
        let Some(agent) = current_actor().as_agent().cloned() else {
            return Ok(());
        };
        let mut folders: Vec<DocumentFolderId> = Vec::new();
        if let Some(folder) = &document.folder_id {
            folders.push(folder.clone());
            folders.extend(self.folders.ancestors(folder).await?);
        }
        let context = document_context(who, document, folders);
        if self.agents.allows(capability, &context, &agent) {
            return Ok(());
        }
        Err(AgentRefusal::HardRule {
            agent: agent.name,
            capability,
        }
        .into())
    }

    /// The cap `agent` would act under on the document or folder `granted`
    /// names; the root when it names neither.
    pub async fn agent_cap_for(
        &self,
        who: &AuthenticatedUser,
        granted: &Granted,
        agent: &AgentActor,
    ) -> Result<EffectiveCap, DocumentsError> {
        let context = match (&granted.document, &granted.folder) {
            (Some(document), _) => {
                let mut folders: Vec<DocumentFolderId> = Vec::new();
                if let Some(folder) = &document.folder_id {
                    folders.push(folder.clone());
                    folders.extend(self.folders.ancestors(folder).await?);
                }
                document_context(who, document, folders)
            },
            (None, folder) => DocumentContext {
                document_id: None,
                folders: folder
                    .iter()
                    .flat_map(|f| {
                        std::iter::once(f.id.clone()).chain(f.ancestor_ids.iter().cloned())
                    })
                    .collect(),
                owner: folder.as_ref().map(|f| f.owner_user_id.clone()),
                name: folder.as_ref().map(|f| f.name.clone()),
                format: None,
                on_behalf_of: who.id.clone(),
            },
        };
        self.effective_cap(&context, agent).await
    }

    /// Whether an agent caller must not see `document` at all: its cap is
    /// `none`. Always `false` for a person; an error hides it.
    pub async fn hidden_from_agent(&self, who: &AuthenticatedUser, document: &Document) -> bool {
        let Some(agent) = current_actor().as_agent().cloned() else {
            return false;
        };
        let cap = async {
            let mut folders: Vec<DocumentFolderId> = Vec::new();
            if let Some(folder) = &document.folder_id {
                folders.push(folder.clone());
                folders.extend(self.folders.ancestors(folder).await?);
            }
            let context = document_context(who, document, folders);
            self.effective_cap(&context, &agent).await
        };
        match cap.await {
            Ok(effective) => effective.cap == AgentCap::None,
            Err(error) => {
                tracing::warn!(
                    document_id = %document.id,
                    error = %error,
                    "Agent cap could not be resolved; the document is withheld from the agent"
                );
                true
            },
        }
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

fn document_context(
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
        on_behalf_of: who.id.clone(),
    }
}

fn context_of(who: &AuthenticatedUser, resolved: &Resolved) -> DocumentContext {
    if let Some(document) = &resolved.document {
        return document_context(who, document, resolved.chain.clone());
    }
    DocumentContext {
        document_id: None,
        folders: resolved.chain.clone(),
        owner: resolved.folder.as_ref().map(|f| f.owner_user_id.clone()),
        name: resolved.folder.as_ref().map(|f| f.name.clone()),
        format: None,
        on_behalf_of: who.id.clone(),
    }
}
