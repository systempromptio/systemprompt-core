//! The reranker step.
//!
//! One [`Reranker`](systemprompt_extension::brain::Reranker) reorders the
//! visible facts between the visibility pass and the budget fill, so it
//! only ever sees facts the reader can see and the fill then takes them
//! in the order it gave.
//!
//! [`merge`] is the pure rule: the ids the reranker returned come first
//! in the returned order (a repeated id counts at its first position),
//! the ids it omitted follow in their fused order, an id it never
//! received is ignored ([`returned_of`] is that filter), and an empty
//! result leaves the order unchanged.
//! `BrainRecallService::rerank` applies the first registered reranker
//! under `reranker_timeout_ms`; an error or a timeout is logged with the
//! reranker's name and leaves the fused order untouched. The receipt
//! records which reranker ran and the rank it gave each injected fact
//! ([`record`]); an injected fact the reranker omitted has no rank.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::time::Duration;

use systemprompt_extension::brain::{RecallRequest, RerankCandidate, RerankQuery};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{RecallRerank, RerankedFact, ScoredFact};

use super::declared::DeclaredRecall;
use super::service::BrainRecallService;
use super::signal;

/// What one reranker made of the visible facts: the ids it returned, in
/// the order it returned them (deduplicated, unknown ids dropped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reranked {
    pub reranker: &'static str,
    pub order: Vec<BrainFactId>,
}

#[must_use]
pub fn merge(visible: Vec<ScoredFact>, returned: &[BrainFactId]) -> Vec<ScoredFact> {
    let order = returned_of(&visible, returned);
    if order.is_empty() {
        return visible;
    }
    let mut first: Vec<Option<ScoredFact>> = vec![None; order.len()];
    let mut rest: Vec<ScoredFact> = Vec::new();
    for scored in visible {
        match order.iter().position(|id| *id == scored.fact.id) {
            Some(at) => first[at] = Some(scored),
            None => rest.push(scored),
        }
    }
    first.into_iter().flatten().chain(rest).collect()
}

#[must_use]
pub fn record(reranked: &Reranked, injected: &[ScoredFact]) -> RecallRerank {
    let ranks = injected
        .iter()
        .filter_map(|scored| {
            let at = reranked.order.iter().position(|id| *id == scored.fact.id)?;
            Some(RerankedFact {
                id: scored.fact.id.clone(),
                rank: u32::try_from(at + 1).unwrap_or(u32::MAX),
            })
        })
        .collect();
    RecallRerank {
        reranker: reranked.reranker.to_owned(),
        ranks,
    }
}

#[must_use]
pub fn returned_of(visible: &[ScoredFact], returned: &[BrainFactId]) -> Vec<BrainFactId> {
    let known: HashSet<&BrainFactId> = visible.iter().map(|scored| &scored.fact.id).collect();
    let mut order: Vec<BrainFactId> = Vec::new();
    for id in returned {
        if known.contains(id) && !order.contains(id) {
            order.push(id.clone());
        }
    }
    order
}

impl BrainRecallService {
    pub(super) async fn rerank(
        &self,
        request: &RecallRequest,
        visible: Vec<ScoredFact>,
        declared: &DeclaredRecall,
    ) -> (Vec<ScoredFact>, Option<Reranked>) {
        let Some(reranker) = self.rerankers.first() else {
            return (visible, None);
        };
        if visible.is_empty() {
            return (visible, None);
        }
        let query = RerankQuery {
            subject_hints: request.subject_hints.clone(),
            asked_kinds: signal::asked_kinds(&request.subject_hints, &declared.kinds),
        };
        let candidates: Vec<RerankCandidate> = visible.iter().map(RerankCandidate::of).collect();
        let timeout = Duration::from_millis(u64::from(declared.retrieval.reranker_timeout_ms));
        let returned =
            match tokio::time::timeout(timeout, reranker.rerank(&query, &candidates)).await {
                Ok(Ok(ids)) => ids,
                Ok(Err(error)) => {
                    tracing::warn!(
                        reranker = reranker.name(),
                        candidates = candidates.len(),
                        error = %error,
                        "Brain reranker failed; the fused order stands"
                    );
                    return (visible, None);
                },
                Err(_) => {
                    tracing::warn!(
                        reranker = reranker.name(),
                        candidates = candidates.len(),
                        timeout_ms = declared.retrieval.reranker_timeout_ms,
                        "Brain reranker timed out; the fused order stands"
                    );
                    return (visible, None);
                },
            };
        let order = returned_of(&visible, &returned);
        if order.is_empty() {
            return (visible, None);
        }
        let outcome = Reranked {
            reranker: reranker.name(),
            order: order.clone(),
        };
        (merge(visible, &order), Some(outcome))
    }
}
