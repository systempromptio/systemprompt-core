//! Brain seams: the outbox sink, the capture pipeline's filters,
//! extractors, validators, importance models and source content, the model
//! client, the read side's store contract, retrievers and injection
//! policies, the gateway-facing [`MemoryRecall`] with its
//! [`SourceAccess`] check, and the measure side's outcome signals and
//! analyses.
//!
//! Producers emit through the injected [`BrainSink`] inside their own
//! transaction. Extensions contribute filters, extractors, validators,
//! importance models, source content, retrievers, injection policies,
//! outcome signals and analyses with the `register_brain_*!` macros; each
//! registration carries a `priority` and a factory over the
//! [`crate::ExtensionContext`], and the matching `discover_*` function
//! returns them sorted by ascending priority. The async traits are held as
//! `Arc<dyn _>` and so use `#[async_trait]`; [`EventFilter`],
//! [`ImportanceModel`] and [`InjectionPolicy`] are pure and synchronous.
//! The sink and the `Database` error variants need the `sqlx` feature.
//!
//! A recalled [`RecallBlock`] renders as the `<team_memory>` system block
//! and names its facts for the `x-systemprompt-memory-ids` header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod errors;
mod recall;
mod registry;
mod render;
mod retrieval;
mod source_access;
mod traits;

pub use errors::{
    BrainAnalysisError, BrainExtractError, BrainModelError, BrainSinkError, BrainValidateError,
    OutcomeSignalError, RecallError, SourceAccessError, SourceContentError,
};
pub use recall::{
    MAX_SUBJECT_HINTS, MemoryRecall, RecallConversation, RecallOutcome, RecallPrincipal,
    RecallReceipt, RecallRequest, ReuseHint, SkipReason,
};
pub use registry::{
    BrainAnalysisRegistration, BrainExtractorRegistration, BrainFilterRegistration,
    BrainImportanceRegistration, BrainInjectionPolicyRegistration, BrainOutcomeSignalRegistration,
    BrainRetrieverRegistration, BrainSourceContentRegistration, BrainValidatorRegistration,
    discover_brain_analyses, discover_brain_extractors, discover_brain_filters,
    discover_brain_importance_models, discover_brain_injection_policies,
    discover_brain_outcome_signals, discover_brain_retrievers, discover_brain_source_contents,
    discover_brain_validators,
};
pub use render::{InjectedFact, RecallBlock, WithheldSummary};
pub use retrieval::{
    AnalysisWindow, BrainAnalysis, FactRecallSource, FactRetriever, InjectionPolicy, OutcomeSignal,
};
pub use source_access::{SourceAccess, SourceVerdict};
pub use systemprompt_models::brain::WithheldReason;
pub use traits::{
    BrainModelClient, EventFilter, FactExtractor, FactValidator, FilterVerdict, ImportanceModel,
    SourceContent, ValidationVerdict,
};
#[cfg(feature = "sqlx")]
pub use traits::{BrainSink, DynBrainSink};
