//! One active `brain_consolidate` run: the declared kinds a consolidator
//! handles, their facts grouped by subject, and each answered candidate
//! entered as a `proposed` fact through the brain sink.
//!
//! Candidates take the entered pathway (`brain.fact.entered`, status
//! `proposed`), so the consumer validates, deduplicates and routes them like
//! a person-entered fact; the fact's source is the job's actor and its
//! `via_agent` names the consolidator. A candidate without a scope inherits
//! the scope of the facts it was built from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::error::BrainError;
use crate::models::FactFilter;
use crate::pipeline::metrics;
use crate::recall::{DeclaredSource, PlaneDeclaredSource};
use crate::repository::BrainRepositories;
use async_trait::async_trait;
use chrono::Utc;
use systemprompt_extension::brain::{
    BrainModelClient, BrainModelError, ConsolidationInput, DynBrainSink, FactConsolidator,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::{
    BRAIN_CONSOLIDATE_PURPOSE, BrainFact, BrainModelRequest, BrainModelResponse, EnteredFact,
    EnteredStatus, FactCandidate, FactStatus, KindDecl,
};

const FACT_SCAN: i64 = 5000;
const MIN_GROUP: usize = 2;

/// Stamps every request a consolidator sends with the consolidation
/// purpose, so the gateway attributes it apart from extraction.
struct PurposedModel {
    inner: Arc<dyn BrainModelClient>,
}

#[async_trait]
impl BrainModelClient for PurposedModel {
    async fn complete(
        &self,
        request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        self.inner
            .complete(request.with_purpose(BRAIN_CONSOLIDATE_PURPOSE))
            .await
    }
}

pub(super) struct ConsolidationRun<'a> {
    pub repos: &'a BrainRepositories,
    pub consolidators: &'a [Arc<dyn FactConsolidator>],
    pub model: Arc<dyn BrainModelClient>,
    pub sink: DynBrainSink,
    pub actor: UserId,
    pub budget_tokens: u64,
}

#[derive(Debug, Default)]
pub(super) struct RunTotals {
    pub entered: u64,
    pub failed: u64,
}

fn grouped(facts: Vec<BrainFact>) -> Vec<Vec<BrainFact>> {
    let mut groups: BTreeMap<(String, String), Vec<BrainFact>> = BTreeMap::new();
    for fact in facts {
        groups
            .entry((fact.subject.kind.clone(), fact.subject.id.clone()))
            .or_default()
            .push(fact);
    }
    groups
        .into_values()
        .filter(|group| group.len() >= MIN_GROUP)
        .collect()
}

fn entered(candidate: FactCandidate, group: &[BrainFact]) -> Option<EnteredFact> {
    let scope = candidate
        .scope
        .or_else(|| group.first().map(|fact| fact.scope.clone()))?;
    Some(EnteredFact {
        kind: candidate.kind,
        subject: candidate.subject,
        fields: candidate.fields,
        scope,
        topics: candidate.topics,
        status: EnteredStatus::Proposed,
        observed_at: None,
    })
}

/// One subject group's consolidator, kind and the model it may call.
struct GroupTarget<'a> {
    consolidator: &'a dyn FactConsolidator,
    kind: &'a KindDecl,
    model: &'a Arc<dyn BrainModelClient>,
}

impl ConsolidationRun<'_> {
    pub(super) async fn execute(&self) -> Result<RunTotals, BrainError> {
        let declared = PlaneDeclaredSource::new(self.repos.clone()).load().await?;
        let model: Arc<dyn BrainModelClient> = Arc::new(PurposedModel {
            inner: Arc::clone(&self.model),
        });
        let mut totals = RunTotals::default();
        for kind in &declared.kinds {
            let Some(consolidator) = self.consolidators.iter().find(|c| c.handles(kind)) else {
                continue;
            };
            let filter = FactFilter::new()
                .with_kind(kind.kind.clone())
                .with_statuses(vec![FactStatus::Confirmed, FactStatus::Proposed])
                .with_limit(FACT_SCAN);
            let facts = self.repos.facts.list(&filter).await?;
            for group in grouped(facts) {
                let target = GroupTarget {
                    consolidator: consolidator.as_ref(),
                    kind,
                    model: &model,
                };
                self.consolidate_group(target, group, &mut totals).await?;
            }
        }
        Ok(totals)
    }

    async fn consolidate_group(
        &self,
        target: GroupTarget<'_>,
        group: Vec<BrainFact>,
        totals: &mut RunTotals,
    ) -> Result<(), BrainError> {
        let GroupTarget {
            consolidator,
            kind,
            model,
        } = target;
        let subject = group[0].subject.clone();
        let input = ConsolidationInput {
            kind: kind.clone(),
            subject,
            facts: group,
            model: Arc::clone(model),
            budget_tokens: self.budget_tokens,
        };
        let candidates = match consolidator.consolidate(&input).await {
            Ok(candidates) => candidates,
            Err(error) => {
                totals.failed += 1;
                metrics::consolidation("failed");
                tracing::warn!(
                    consolidator = consolidator.name(),
                    kind = %kind.kind,
                    error = %error,
                    "Brain consolidation failed"
                );
                return Ok(());
            },
        };
        for candidate in candidates {
            let Some(fact) = entered(candidate, &input.facts) else {
                continue;
            };
            self.enter(consolidator.name(), &fact).await?;
            totals.entered += 1;
            metrics::consolidation("entered");
        }
        Ok(())
    }

    async fn enter(
        &self,
        consolidator: &'static str,
        fact: &EnteredFact,
    ) -> Result<(), BrainError> {
        let event = fact
            .to_event(self.actor.clone(), Utc::now())?
            .with_actor_is_agent(true)
            .with_via_agent(consolidator);
        let mut tx = self.repos.write_pool().begin().await?;
        self.sink.emit(&mut tx, &event).await?;
        tx.commit().await?;
        Ok(())
    }
}
