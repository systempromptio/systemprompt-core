//! [`OutcomeService`]: records the outcomes of one gateway response.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use systemprompt_extension::brain::OutcomeSignal;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::brain::{BrainOutcome, CITED_IN_RESPONSE, OutcomeObservation};

use super::cited::cited_facts;
use crate::error::BrainResult;
use crate::repository::{OutcomeRepository, RecallRepository};

/// Observes assistant responses for the facts their recall injected.
#[derive(Clone)]
pub struct OutcomeService {
    recalls: Arc<RecallRepository>,
    outcomes: Arc<OutcomeRepository>,
    signals: Vec<Arc<dyn OutcomeSignal>>,
}

impl std::fmt::Debug for OutcomeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutcomeService")
            .field("signals", &self.signals.len())
            .finish_non_exhaustive()
    }
}

fn injected_by(observation: &OutcomeObservation, outcome: &BrainOutcome) -> bool {
    outcome.recall_id == observation.recall.recall_id
        && observation
            .recall
            .injected
            .iter()
            .any(|fact| fact.id == outcome.fact_id)
}

impl OutcomeService {
    #[must_use]
    pub const fn new(
        recalls: Arc<RecallRepository>,
        outcomes: Arc<OutcomeRepository>,
        signals: Vec<Arc<dyn OutcomeSignal>>,
    ) -> Self {
        Self {
            recalls,
            outcomes,
            signals,
        }
    }

    pub async fn observe_response(
        &self,
        ai_request_id: &AiRequestId,
        assistant_text: &str,
    ) -> BrainResult<u32> {
        let Some(recall) = self.recalls.audit_for_request(ai_request_id).await? else {
            return Ok(0);
        };
        if recall.injected.is_empty() {
            return Ok(0);
        }
        let observed_at = Utc::now();
        let mut outcomes: Vec<BrainOutcome> = cited_facts(assistant_text, &recall.injected)
            .into_iter()
            .map(|fact_id| BrainOutcome {
                recall_id: recall.recall_id.clone(),
                fact_id,
                signal: CITED_IN_RESPONSE.to_owned(),
                value: 1.0,
                observed_at,
            })
            .collect();
        let observation = OutcomeObservation {
            ai_request_id: ai_request_id.clone(),
            recall,
            response_text: Some(assistant_text.to_owned()),
            observed_at,
        };
        let observed = join_all(self.signals.iter().map(|s| s.observe(&observation))).await;
        for (signal, result) in self.signals.iter().zip(observed) {
            match result {
                Ok(found) => {
                    outcomes.extend(found.into_iter().filter(|o| injected_by(&observation, o)));
                },
                Err(error) => tracing::warn!(
                    signal = signal.name(),
                    ai_request_id = %ai_request_id,
                    error = %error,
                    "Brain outcome signal failed; its outcomes are not recorded"
                ),
            }
        }
        let mut written = 0u32;
        for outcome in &outcomes {
            if self.outcomes.record_detached(outcome).await? {
                written += 1;
            }
        }
        Ok(written)
    }
}
