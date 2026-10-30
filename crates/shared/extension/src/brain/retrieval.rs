//! The read-side store contract, extension retrievers and injection
//! policies, and the measure-side outcome signals and analyses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{BrainFactId, GatewayConversationId};
use systemprompt_models::brain::{
    BrainFact, BrainOutcome, InjectionPlan, OutcomeObservation, ProposalDraft, RecallCtx,
    RecallQuery, ScoredFact, StoredBlock,
};

use super::errors::{BrainAnalysisError, OutcomeSignalError, RecallError};
use super::recall::RecallReceipt;

/// The fact store as recall reads it; every tier is scope-filtered in SQL
/// for the context's principal. Held as `Arc<dyn FactRecallSource>`.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactRecallSource: Send + Sync {
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
