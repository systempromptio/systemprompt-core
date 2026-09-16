//! Materialize bounded development suggestions as immutable candidate
//! revisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_evaluation::campaigns::CampaignPolicy;
use systemprompt_evaluation::campaigns::suggestions::RetainedSuggestion;
use systemprompt_identifiers::{ResourceRevisionId, UserId};
use systemprompt_marketplace::managed::TextCandidate;

use super::{OptimizationError, SkillOptimizationOrchestrator};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedChanges {
    files: Vec<ProposedFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedFile {
    path: String,
    content: String,
}

impl SkillOptimizationOrchestrator {
    pub(super) async fn apply_suggestion(
        &self,
        owner: &UserId,
        policy: &CampaignPolicy,
        previous: &systemprompt_evaluation::experiments::VariantSpec,
        suggestion: &RetainedSuggestion,
    ) -> Result<ResourceRevisionId, OptimizationError> {
        let edits: ProposedChanges = serde_json::from_value(suggestion.proposed_changes.clone())?;
        if edits.files.is_empty() || edits.files.len() > 16 {
            return Err(OptimizationError::Source(
                "Suggestions require 1–16 bounded file edits".to_owned(),
            ));
        }
        let workspace = self
            .evaluations
            .evidence
            .get_managed_workspace(owner, &previous.skill_bundle_digest)
            .await?;
        let mut revision = workspace.managed_revision_id;
        let previous_content = self
            .managed
            .get_revision_bundle(owner, &revision)
            .await?
            .content_digest()?;
        if self.managed.revision_resource(owner, &revision).await? != policy.resource_id {
            return Err(OptimizationError::Source(
                "Suggestion candidate belongs to another resource".to_owned(),
            ));
        }
        for edit in edits.files {
            revision = self
                .managed
                .create_text_candidate(
                    owner,
                    &revision,
                    &TextCandidate {
                        path: edit.path,
                        content: edit.content,
                        rationale: suggestion.hypothesis.clone(),
                    },
                )
                .await?;
        }
        if self
            .managed
            .get_revision_bundle(owner, &revision)
            .await?
            .content_digest()?
            == previous_content
        {
            return Err(OptimizationError::Source(
                "Suggestion does not change the retained candidate content".to_owned(),
            ));
        }
        Ok(revision)
    }
}
