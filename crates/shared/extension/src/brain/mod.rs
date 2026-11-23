//! Brain seams, from capture through recall to measurement.
//!
//! They are the outbox sink, the capture pipeline's filters, extractors,
//! validators, importance models and source content, the model client, the
//! read side's store contract, retrievers, reranker and injection
//! policies, the
//! gateway-facing [`MemoryRecall`] with its [`SourceAccess`] check, and the
//! measure side's outcome signals and analyses.
//!
//! Producers emit through the injected [`BrainSink`] inside their own
//! transaction. Extensions contribute filters, extractors, validators,
//! importance models, source content, retrievers, injection policies,
//! outcome signals, analyses and consolidators through the brain capability's
//! slot, as factories over `Resolved`. The async traits are held as
//! `Arc<dyn _>` and so use `#[async_trait]`; [`EventFilter`],
//! [`ImportanceModel`] and [`InjectionPolicy`] are pure and synchronous.
//! The sink and the `Database` error variants need the `sqlx` feature.
//!
//! A recalled [`RecallBlock`] renders as the `<team_memory>` system block
//! and names its facts for the `x-systemprompt-memory-ids` header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod consolidate;
mod errors;
mod recall;
mod render;
mod rerank;
mod retrieval;
mod source_access;
#[cfg(feature = "mcp")]
mod toolset;
mod traits;

pub use consolidate::{ConsolidationInput, FactConsolidator};
pub use errors::{
    BrainAnalysisError, BrainConsolidateError, BrainExtractError, BrainModelError, BrainSinkError,
    BrainValidateError, OutcomeSignalError, RecallError, RerankError, SourceAccessError,
    SourceContentError,
};
pub use recall::{
    MAX_SUBJECT_HINTS, MemoryRecall, RecallConversation, RecallObservation, RecallOutcome,
    RecallPrincipal, RecallReceipt, RecallRequest, ReuseHint, SkipReason,
};
pub use render::{InjectedFact, RecallBlock, WithheldSummary};
pub use rerank::{RerankCandidate, RerankQuery, Reranker};
pub use retrieval::{
    AnalysisWindow, BrainAnalysis, FactRecallSource, FactRetriever, InjectionPolicy, OutcomeSignal,
    TierIds,
};
pub use source_access::{SourceAccess, SourceVerdict};
pub use systemprompt_models::brain::WithheldReason;
#[cfg(feature = "mcp")]
pub use toolset::{BRAIN_TOOLSET, BrainToolsetSource, DynBrainToolsetSource};
pub use traits::{
    BrainConsumerStart, BrainModelClient, DynBrainConsumerStart, EventFilter, FactExtractor,
    FactValidator, FilterVerdict, ImportanceModel, SourceContent, ValidationVerdict,
};
#[cfg(feature = "sqlx")]
pub use traits::{BrainSink, DynBrainSink};
