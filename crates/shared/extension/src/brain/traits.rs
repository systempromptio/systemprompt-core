//! The capture-side brain seams: the outbox sink, event filters,
//! extractors, validators, importance models, source content and the model
//! client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "sqlx")]
use std::sync::Arc;

#[cfg(feature = "sqlx")]
use systemprompt_identifiers::EventOutboxId;
use systemprompt_identifiers::FactKindId;
#[cfg(feature = "sqlx")]
use systemprompt_models::brain::BrainEvent;
use systemprompt_models::brain::{
    BrainEnvelope, BrainFact, BrainModelRequest, BrainModelResponse, ExtractionInput,
    ExtractionOutput, FactCandidate, FactSource, ImportanceConfig, KindDecl, RecallCtx, SourceText,
};

#[cfg(feature = "sqlx")]
use super::errors::BrainSinkError;
use super::errors::{BrainExtractError, BrainModelError, BrainValidateError, SourceContentError};

/// Appends a brain event to the outbox on the caller's connection;
/// injected at composition roots as [`DynBrainSink`].
///
/// A server with the brain disabled or not built injects a sink whose
/// `is_available` is `false` and whose `emit` returns
/// [`BrainSinkError::Unavailable`]; producers check availability before
/// their transaction and skip recording rather than roll back.
///
/// `#[async_trait]` keeps the trait dyn-compatible: producers hold it as
/// `Arc<dyn BrainSink>`.
#[cfg(feature = "sqlx")]
#[async_trait::async_trait]
pub trait BrainSink: Send + Sync {
    fn is_available(&self) -> bool;

    async fn emit(
        &self,
        conn: &mut sqlx::PgConnection,
        event: &BrainEvent,
    ) -> Result<EventOutboxId, BrainSinkError>;
}

#[cfg(feature = "sqlx")]
pub type DynBrainSink = Arc<dyn BrainSink>;

/// An event filter's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterVerdict {
    Gather { kinds: Vec<FactKindId> },
    Ignore,
    Abstain,
}

/// Decides, purely and before any model call, whether an event is worth
/// reading; held as `Arc<dyn EventFilter>`. Filters run by priority and
/// the first `Gather` or `Ignore` wins; an empty `kinds` gathers every
/// kind.
pub trait EventFilter: Send + Sync {
    fn name(&self) -> &'static str;

    fn decide(&self, envelope: &BrainEnvelope) -> FilterVerdict;
}

/// Turns an event's source into candidates for one kind; held as
/// `Arc<dyn FactExtractor>`.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactExtractor: Send + Sync {
    fn name(&self) -> &'static str;

    fn handles(&self, kind: &KindDecl) -> bool;

    async fn extract(&self, input: &ExtractionInput)
    -> Result<ExtractionOutput, BrainExtractError>;
}

/// A validator's decision on one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationVerdict {
    Accept,
    Reject { reason: String },
    Dispute { reason: String },
}

/// Checks a candidate after core's schema validation; held as
/// `Arc<dyn FactValidator>`.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait FactValidator: Send + Sync {
    fn name(&self) -> &'static str;

    async fn validate(
        &self,
        candidate: &FactCandidate,
        kind: &KindDecl,
        text: Option<&SourceText>,
    ) -> Result<ValidationVerdict, BrainValidateError>;
}

/// Scores importance purely: a candidate's stored base on write and a
/// stored fact's in-context score per request; held as
/// `Arc<dyn ImportanceModel>`. `None` abstains to the next model and
/// finally to core's formula.
pub trait ImportanceModel: Send + Sync {
    fn name(&self) -> &'static str;

    fn base(&self, candidate: &FactCandidate, config: &ImportanceConfig) -> Option<f64>;

    fn in_context(
        &self,
        fact: &BrainFact,
        ctx: &RecallCtx,
        config: &ImportanceConfig,
    ) -> Option<f64>;
}

/// Reads the text of the sources it handles; held as
/// `Arc<dyn SourceContent>`. `Ok(None)` means the source is gone or
/// unreadable, and the candidate is rejected.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait SourceContent: Send + Sync {
    fn name(&self) -> &'static str;

    fn handles(&self, source: &FactSource) -> bool;

    async fn fetch(&self, source: &FactSource) -> Result<Option<SourceText>, SourceContentError>;
}

/// Sends a model call through the in-process gateway client, metered,
/// guarded and audited like any other request; held as
/// `Arc<dyn BrainModelClient>`.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait BrainModelClient: Send + Sync {
    async fn complete(
        &self,
        request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError>;
}
