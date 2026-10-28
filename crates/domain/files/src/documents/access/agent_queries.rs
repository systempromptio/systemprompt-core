//! What the rest of the document service asks the agent step directly: the
//! hard rule for one capability, the cap on a granted target, and whether a
//! document is hidden from the calling agent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::AuthenticatedUser;

use super::agent::{document_context, folder_context};
use super::service::{DocumentAccessService, Granted};
use crate::documents::agents::{
    AgentActor, AgentCap, AgentCapability, AgentRefusal, EffectiveCap, current_actor,
};
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;

impl DocumentAccessService {
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

    /// The cap `agent` would act under on the document or folder `granted`
    /// names; the root when it names neither.
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

    /// Whether an agent caller must not see `document` at all: its cap is
    /// `none`. Always `false` for a person; an error hides it.
    pub async fn hidden_from_agent(&self, who: &AuthenticatedUser, document: &Document) -> bool {
        let Some(agent) = current_actor().as_agent().cloned() else {
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
