//! Development-only suggestions retained by the metered evaluator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use crate::models::SuggestionStatus;
use crate::repository::experiments::EvaluationLifecycleRepository;
use serde::Serialize;
use systemprompt_identifiers::{EvalExperimentId, EvalSuggestionId, UserId};

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct RetainedSuggestion {
    pub id: EvalSuggestionId,
    pub hypothesis: String,
    pub status: SuggestionStatus,
    // JSON: the evaluator's existing suggestion protocol permits arbitrary file changes.
    pub proposed_changes: serde_json::Value,
    // JSON: retained evaluator evidence references use the existing evidence protocol.
    pub originating_evidence: serde_json::Value,
}

impl EvaluationLifecycleRepository {
    pub async fn list_suggestions(
        &self,
        owner: &UserId,
        experiment: &EvalExperimentId,
    ) -> Result<Vec<RetainedSuggestion>> {
        let rows = sqlx::query!("SELECT s.id,s.hypothesis,s.status,s.proposed_changes,s.originating_evidence FROM eval_suggestions s WHERE s.owner_id=$1 AND s.experiment_id=$2 AND NOT EXISTS(SELECT 1 FROM unnest(s.supporting_execution_ids) AS supporting(execution_id) LEFT JOIN eval_executions x ON x.id=supporting.execution_id LEFT JOIN eval_experiments e ON e.id=x.experiment_id LEFT JOIN eval_resource_revisions c ON c.id=x.case_revision_id AND c.owner_id=e.owner_id WHERE e.owner_id IS DISTINCT FROM s.owner_id OR e.id IS DISTINCT FROM s.experiment_id OR c.content->'content'->>'partition' IS DISTINCT FROM 'development') ORDER BY s.created_at,s.id LIMIT 100", owner.as_str(), experiment.as_str()).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(RetainedSuggestion {
                    id: EvalSuggestionId::new(row.id),
                    hypothesis: row.hypothesis,
                    status: SuggestionStatus::parse(&row.status)?,
                    proposed_changes: row.proposed_changes,
                    originating_evidence: row.originating_evidence,
                })
            })
            .collect()
    }
}
