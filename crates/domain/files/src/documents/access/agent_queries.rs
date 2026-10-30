//! What the rest of the document service asks the agent step directly: the
//! hard rule for one capability, the cap on a granted target, and whether a
//! document is hidden from the calling agent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;

use super::agent::{combine, document_context, folder_context};
use super::service::{DocumentAccessService, Granted};
use crate::documents::agents::{
    AgentAccessPolicy, AgentActor, AgentCap, AgentCapability, AgentRefusal, CapSource,
    DocumentActor, EffectiveCap, scoped_actor,
};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::repository::{AgentReadRecord, FolderChain};

impl DocumentAccessService {
    pub async fn require_agent_capability(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        capability: AgentCapability,
    ) -> Result<(), DocumentsError> {
        let Some(agent) = self.actor().as_agent().cloned() else {
            return Ok(());
        };
        let mut context = document_context(who, document, self.chain_of(document).await?);
        self.effective_cap(&mut context, &agent).await?;
        if self.agents.allows(capability, &context, &agent).await {
            return Ok(());
        }
        Err(AgentRefusal::HardRule {
            agent: agent.name,
            capability,
        }
        .into())
    }

    pub async fn agent_cap_for(
        &self,
        who: &AuthenticatedUser,
        granted: &Granted,
        agent: &AgentActor,
    ) -> Result<EffectiveCap, DocumentsError> {
        let mut context = match (&granted.document, &granted.folder) {
            (Some(document), _) => document_context(who, document, self.chain_of(document).await?),
            (None, folder) => {
                let chain = folder
                    .iter()
                    .flat_map(|f| {
                        std::iter::once(f.id.clone()).chain(f.ancestor_ids.iter().rev().cloned())
                    })
                    .collect();
                folder_context(who, folder.as_ref(), chain)
            },
        };
        self.effective_cap(&mut context, agent).await
    }

    #[must_use]
    pub fn with_unscoped_actor(mut self, actor: DocumentActor) -> Self {
        self.unscoped = actor;
        self
    }

    #[must_use]
    pub fn actor(&self) -> DocumentActor {
        scoped_actor().unwrap_or_else(|| self.unscoped.clone())
    }

    #[must_use]
    pub fn with_agent_policy(mut self, policy: Arc<dyn AgentAccessPolicy>) -> Self {
        self.agents = policy;
        self
    }

    pub async fn redact_source(
        &self,
        who: &AuthenticatedUser,
        effective: EffectiveCap,
    ) -> EffectiveCap {
        let fallback = EffectiveCap {
            source: CapSource::InheritedOverride,
            ..effective.clone()
        };
        let Ok(pairs) = self.subject_pairs(who).await else {
            return fallback;
        };
        self.hide_unviewable(who, effective, &pairs)
            .await
            .unwrap_or(fallback)
    }

    pub async fn hidden_from_agent_many(
        &self,
        who: &AuthenticatedUser,
        documents: &[&Document],
        trace: &TraceId,
    ) -> Vec<bool> {
        let Some(agent) = self.actor().as_agent().cloned() else {
            return vec![false; documents.len()];
        };
        match self.caps_of_many(who, documents, &agent).await {
            Ok(caps) => {
                for (document, cap) in documents.iter().zip(&caps) {
                    if *cap == AgentCap::None || *cap >= AgentCap::Edit {
                        continue;
                    }
                    let record = AgentReadRecord {
                        document_id: &document.id,
                        user_id: &who.id,
                        agent: &agent.name,
                        attended: agent.attended,
                        action: "list",
                        cap: *cap,
                        cap_source: "listing",
                        trace_id: trace,
                    };
                    if let Err(error) = self.documents.insert_agent_read(record).await {
                        tracing::warn!(
                            document_id = %document.id,
                            error = %error,
                            "An agent listing read could not be audited"
                        );
                    }
                }
                caps.into_iter().map(|cap| cap == AgentCap::None).collect()
            },
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Agent caps could not be resolved; the page is withheld from the agent"
                );
                vec![true; documents.len()]
            },
        }
    }

    async fn caps_of_many(
        &self,
        who: &AuthenticatedUser,
        documents: &[&Document],
        agent: &AgentActor,
    ) -> Result<Vec<AgentCap>, DocumentsError> {
        let ids: Vec<DocumentId> = documents.iter().map(|d| d.id.clone()).collect();
        let mut folders: Vec<DocumentFolderId> = documents
            .iter()
            .filter_map(|d| d.folder_id.clone())
            .collect();
        folders.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        folders.dedup();
        let mut facts = self.documents.agent_facts_of(&ids).await?;
        let chains = self.documents.folder_chains(&folders).await?;
        let mut caps = Vec::with_capacity(documents.len());
        for document in documents {
            let own = facts.remove(&document.id).unwrap_or_default();
            let chain = match document.folder_id.as_ref() {
                None => FolderChain::default(),
                Some(folder) => {
                    let Some(chain) = chains.get(folder) else {
                        tracing::warn!(
                            document_id = %document.id,
                            folder_id = %folder,
                            "Folder chain missing; the document is withheld from the agent"
                        );
                        caps.push(AgentCap::None);
                        continue;
                    };
                    chain.clone()
                },
            };
            let mut context = document_context(who, document, chain.chain);
            context.created_by_agent = own.created_by_agent;
            let policy_cap = self.agents.cap(&context, agent).await;
            let mut overrides = own.overrides;
            overrides.extend(chain.overrides);
            caps.push(combine(policy_cap, self.agents.name(), overrides).cap);
        }
        Ok(caps)
    }

    pub async fn hidden_from_agent(&self, who: &AuthenticatedUser, document: &Document) -> bool {
        let Some(agent) = self.actor().as_agent().cloned() else {
            return false;
        };
        let cap = async {
            let mut context = document_context(who, document, self.chain_of(document).await?);
            self.effective_cap(&mut context, &agent).await
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
}
