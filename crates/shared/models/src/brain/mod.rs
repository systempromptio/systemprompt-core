//! Brain models: the organisational memory's events, facts, declarations
//! and recall values.
//!
//! Producers append a [`BrainEvent`] to the outbox for the
//! [`BRAIN_CONSUMER`]; the consumer wraps it in a [`BrainEnvelope`], and
//! extractors turn it into [`FactCandidate`]s that become stored
//! [`BrainFact`]s with append-only [`FactRevision`]s. A fact's statement is
//! rendered from its kind's template by [`render_statement`]; its
//! [`identity_hash`] keys revisions of the same fact. Kinds, topics,
//! interests, filters, importance, retrieval and workflow profiles are
//! declared in YAML ([`declarations`]) and checked by [`validate`]; the
//! [`PipelineVersionInputs`] digest names the config a fact was built
//! under. The read side ranks [`ScoredFact`]s into an [`InjectionPlan`]
//! and audits it as a [`RecallAudit`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod candidate;
pub mod declarations;
pub mod enter;
pub mod envelope;
pub mod event;
pub mod extraction;
pub mod fact;
pub mod kind_decl;
pub mod outcome;
pub mod proposal;
pub mod recall;
pub mod revision;
pub mod scope;
pub mod source;
pub mod statement;
pub mod status;
pub mod subject;
pub mod version;

pub use candidate::{FactCandidate, FactCandidateBuilder};
pub use declarations::{
    ActorFilter, BaseImportance, BrainDeclError, BrainDocs, FilterDecl, FiltersDoc, Granularity,
    ImportanceConfig, InContextImportance, InterestDecl, InterestDelivery, InterestMatch,
    ModelHint, Recency, RetrievalDoc, TierWeights, TopicDecl, TopicsDoc, UsageWeights,
    WorkflowProfileDecl, validate,
};
pub use enter::{EnteredFact, EnteredStatus};
pub use envelope::{BrainEnvelope, Pathway, PathwayWeight};
pub use event::{
    BRAIN_CONSUMER, BRAIN_EVENT_VERSION, BrainEvent, BrainEventKind, BrainEventKindError,
};
pub use extraction::{
    BRAIN_EXTRACT_PURPOSE, BrainModelRequest, BrainModelResponse, BrainPrompt, ExtractionInput,
    ExtractionOutput, ExtractionRecord, SourceNode, SourceText, StrictJsonSchema,
};
pub use fact::BrainFact;
pub use kind_decl::{ExtractorDecl, FieldDecl, FieldType, KindDecl, Materiality, SUBJECT_FIELD};
pub use outcome::{BrainOutcome, CITED_IN_RESPONSE, OutcomeObservation, USER_CORRECTED_FACT};
pub use proposal::{BrainProposal, ProposalDraft, ProposalKind, ProposalStatus};
pub use recall::{
    FactRef, InjectionPlan, RecallAudit, RecallCtx, RecallQuery, RetrievalTier, ScoredFact,
    TierRank, WithheldFact, WithheldReason,
};
pub use revision::{FactRevision, RevisionChange};
pub use scope::{FactScope, ScopeLevel, UnknownScopeLevel};
pub use source::{
    DOCUMENT_SUBJECT_KIND, EXTERNAL_SUBJECT_PREFIX, FactSource, SourceClass, SourceSpan,
};
pub use statement::{
    StatementError, identity_hash, placeholders, render_statement, statement_hash,
};
pub use status::{FactStatus, UnknownFactStatus};
pub use subject::{SubjectKey, SubjectKeyError};
pub use version::{BRAIN_CODE_VERSION, PipelineVersionInputs};
