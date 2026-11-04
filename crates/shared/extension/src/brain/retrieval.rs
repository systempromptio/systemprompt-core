//! The read-side store contract, extension retrievers and injection
//! policies, and the measure-side outcome signals and analyses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{BrainFactId, GatewayConversationId};
use systemprompt_models::brain::{
    BrainFact, BrainOutcome, InjectionPlan, OutcomeObservation, ProposalDraft, RecallCtx,
    RecallQuery, RetrievalTier, ScoredFact, StoredBlock, WithheldReason,
};

use super::errors::{BrainAnalysisError, OutcomeSignalError, RecallError};
use super::recall::RecallReceipt;

/// The fact ids one retrieval tier returned, in its rank order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierIds {
    pub tier: RetrievalTier,
    pub ids: Vec<BrainFactId>,
}

impl TierIds {
    #[must_use]
    pub const fn new(tier: RetrievalTier, ids: Vec<BrainFactId>) -> Self {
        Self { tier, ids }
    }
}

fn ids_of(tier: RetrievalTier, facts: Vec<BrainFact>) -> TierIds {
    TierIds::new(tier, facts.into_iter().map(|fact| fact.id).collect())
}

/// The fact store as recall reads it.
///
/// Every tier is scope-filtered in SQL for the context's principal. Held
/// as `Arc<dyn FactRecallSource>`. Recall reads the five core tiers as ids
/// through `tier_ids` and loads the fused facts once; the fact-returning
/// methods remain for callers that read one tier.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactRecallSource: Send + Sync {
    async fn tier_ids(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
        pinned_limit: u32,
    ) -> Result<Vec<TierIds>, RecallError> {
        Ok(vec![
            ids_of(RetrievalTier::Pinned, self.pinned(ctx, pinned_limit).await?),
            ids_of(RetrievalTier::Subject, self.by_subject(query, ctx).await?),
            ids_of(RetrievalTier::Linked, self.by_linked(query, ctx).await?),
            ids_of(RetrievalTier::Context, self.by_context(query, ctx).await?),
            ids_of(RetrievalTier::FullText, self.full_text(query, ctx).await?),
        ])
    }

    async fn pinned(&self, ctx: &RecallCtx, limit: u32) -> Result<Vec<BrainFact>, RecallError>;

    async fn by_subject(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError>;

    async fn by_context(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError>;

    async fn by_linked(
        &self,
        _query: &RecallQuery,
        _ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError> {
        Ok(Vec::new())
    }

    async fn full_text(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFact>, RecallError>;

    async fn stored_block(
        &self,
        conversation: &GatewayConversationId,
    ) -> Result<Option<StoredBlock>, RecallError>;

    async fn record_recall(&self, receipt: &RecallReceipt) -> Result<(), RecallError>;

    async fn withheld_counts(
        &self,
        _ctx: &RecallCtx,
    ) -> Result<BTreeMap<WithheldReason, u32>, RecallError> {
        Ok(BTreeMap::new())
    }
}

/// An extension retrieval tier (vector search, an external index, a
/// reranker); held as `Arc<dyn FactRetriever>`. Returns fact ids in rank
/// order.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactRetriever: Send + Sync {
    fn name(&self) -> &'static str;

    async fn retrieve(
        &self,
        query: &RecallQuery,
        ctx: &RecallCtx,
    ) -> Result<Vec<BrainFactId>, RecallError>;
}

/// Chooses which ranked, visible facts fill the budget, purely; held as
/// `Arc<dyn InjectionPolicy>`. `None` abstains to the next policy and
/// finally to core's quota-and-budget fill.
pub trait InjectionPolicy: Send + Sync {
    fn name(&self) -> &'static str;

    fn plan(
        &self,
        ranked: &[ScoredFact],
        ctx: &RecallCtx,
        budget_tokens: u32,
    ) -> Option<InjectionPlan>;
}

/// Observes what happened after a recall; held as
/// `Arc<dyn OutcomeSignal>`.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait OutcomeSignal: Send + Sync {
    fn name(&self) -> &'static str;

    async fn observe(
        &self,
        observation: &OutcomeObservation,
    ) -> Result<Vec<BrainOutcome>, OutcomeSignalError>;
}

/// The history window an analysis reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisWindow {
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

/// Drafts proposals from measured history; held as
/// `Arc<dyn BrainAnalysis>`. Nothing it drafts applies itself.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait BrainAnalysis: Send + Sync {
    fn name(&self) -> &'static str;

    async fn analyse(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<ProposalDraft>, BrainAnalysisError>;
}
