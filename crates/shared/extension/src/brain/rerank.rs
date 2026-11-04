//! The reranker seam: an extension reorders the facts a recall may inject.
//!
//! Recall runs one [`Reranker`] after the visibility pass and before the
//! budget fill, so a reranker only ever sees facts the reader can see.
//! It receives every visible candidate with its fused score, the tiers
//! that found it, its kind and its statement, and returns an ordering of
//! a subset of those ids. The ids it returns fill first in that order;
//! the ids it omits follow in their fused order; an id it did not
//! receive is ignored. An error or a timeout (`retrieval.yaml`
//! `reranker_timeout_ms`) leaves the fused order untouched and is logged
//! with the reranker's name. Only the first registered reranker is
//! applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_models::brain::{RetrievalTier, ScoredFact};

use super::errors::RerankError;

/// What the message asked: its subject hints and the kinds it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankQuery {
    pub subject_hints: String,
    pub asked_kinds: Vec<FactKindId>,
}

/// One visible fact as the reranker sees it: its id, fused score, the
/// tiers that found it, its kind and its statement.
#[derive(Debug, Clone, PartialEq)]
pub struct RerankCandidate {
    pub id: BrainFactId,
    pub score: f64,
    pub tiers: Vec<RetrievalTier>,
    pub kind: FactKindId,
    pub statement: String,
}

impl RerankCandidate {
    #[must_use]
    pub fn of(scored: &ScoredFact) -> Self {
        Self {
            id: scored.fact.id.clone(),
            score: scored.score,
            tiers: scored.ranks.iter().map(|rank| rank.tier).collect(),
            kind: scored.fact.kind.clone(),
            statement: scored.fact.statement.clone(),
        }
    }
}

/// Reorders the visible candidates of one recall; held as
/// `Arc<dyn Reranker>`. Returns the ids to fill first, in rank order;
/// an omitted id keeps its fused order after them.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait Reranker: Send + Sync {
    fn name(&self) -> &'static str;

    async fn rerank(
        &self,
        query: &RerankQuery,
        candidates: &[RerankCandidate],
    ) -> Result<Vec<BrainFactId>, RerankError>;
}
