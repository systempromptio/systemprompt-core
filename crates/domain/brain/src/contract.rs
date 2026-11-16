//! [`BrainSlot`]: the brain's contribution point for its eleven seams.
//!
//! A member contributes a capture filter, extractor, validator, importance
//! model or source reader, a recall retriever, reranker, injection policy or
//! outcome signal, an analysis or a consolidator with
//! `c.slot::<BrainSlot>()` and the matching builder method; each method
//! stores a factory and runs nothing. Admission drops a disabled member's
//! builder before any factory runs, and the kernel then builds the slot
//! once: every factory is called against the resolved state, in composition
//! order, and the result is the [`BrainSeams`] value the kernel inserts as
//! `Arc<BrainSeams>`. The `brain` member always opens the slot, so the built
//! seams are present exactly when that member is admitted.
//!
//! After the contributed seams the build appends the core defaults other
//! members provide as [`CoreSeam`]s: the document source reader `files`
//! provides, after the contributed source readers; the user-role audience
//! resolver `users` provides, as recall's audience resolver; and the
//! document source access `files` provides, as the one source access recall
//! and the service judge a fact's source by. A default whose owner is not
//! admitted is absent; with no source access the brain hides every sourced
//! fact. The capture pipeline takes the five capture seams
//! ([`BrainSeams::pipeline`]); recall takes the four recall seams ahead of
//! its own; the analysis job runs the contributed analyses after the core
//! ones; the consolidation job runs the contributed consolidators. A factory
//! that fails, or reads a value nothing provides, refuses the build: a seam
//! is never silently left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_extension::brain::{FactConsolidator, Reranker, SourceAccess};
use systemprompt_extension::{
    BrainAnalysis, EventFilter, FactExtractor, FactRetriever, FactValidator, Factory,
    ImportanceModel, InjectionPolicy, NotificationAudienceResolver, OutcomeSignal, Resolved, Slot,
    SlotEntry, SlotError, SlotOrder, SourceContent, build_factories,
};
use systemprompt_traits::CoreSeam;
use systemprompt_traits::core_seam::{DocumentSourceAccess, DocumentSourceContent, RoleAudience};

use crate::pipeline::PipelineSeams;

const SLOT: &str = "brain";

type Factories<T> = Vec<Box<dyn Factory<Arc<T>>>>;

/// The brain's contribution point, owned by the `brain` capability.
#[derive(Debug, Clone, Copy)]
pub struct BrainSlot;

/// One member's brain seams, stored until the slot is built.
#[derive(Default)]
pub struct BrainSlotBuilder {
    filters: Factories<dyn EventFilter>,
    extractors: Factories<dyn FactExtractor>,
    validators: Factories<dyn FactValidator>,
    importance: Factories<dyn ImportanceModel>,
    sources: Factories<dyn SourceContent>,
    retrievers: Factories<dyn FactRetriever>,
    rerankers: Factories<dyn Reranker>,
    injection: Factories<dyn InjectionPolicy>,
    outcome_signals: Factories<dyn OutcomeSignal>,
    analyses: Factories<dyn BrainAnalysis>,
    consolidators: Factories<dyn FactConsolidator>,
}

impl BrainSlotBuilder {
    pub fn filter<F: Factory<Arc<dyn EventFilter>>>(&mut self, f: F) -> &mut Self {
        self.filters.push(Box::new(f));
        self
    }

    pub fn extractor<F: Factory<Arc<dyn FactExtractor>>>(&mut self, f: F) -> &mut Self {
        self.extractors.push(Box::new(f));
        self
    }

    pub fn validator<F: Factory<Arc<dyn FactValidator>>>(&mut self, f: F) -> &mut Self {
        self.validators.push(Box::new(f));
        self
    }

    pub fn importance<F: Factory<Arc<dyn ImportanceModel>>>(&mut self, f: F) -> &mut Self {
        self.importance.push(Box::new(f));
        self
    }

    pub fn source_content<F: Factory<Arc<dyn SourceContent>>>(&mut self, f: F) -> &mut Self {
        self.sources.push(Box::new(f));
        self
    }

    pub fn retriever<F: Factory<Arc<dyn FactRetriever>>>(&mut self, f: F) -> &mut Self {
        self.retrievers.push(Box::new(f));
        self
    }

    pub fn reranker<F: Factory<Arc<dyn Reranker>>>(&mut self, f: F) -> &mut Self {
        self.rerankers.push(Box::new(f));
        self
    }

    pub fn injection_policy<F: Factory<Arc<dyn InjectionPolicy>>>(&mut self, f: F) -> &mut Self {
        self.injection.push(Box::new(f));
        self
    }

    pub fn outcome_signal<F: Factory<Arc<dyn OutcomeSignal>>>(&mut self, f: F) -> &mut Self {
        self.outcome_signals.push(Box::new(f));
        self
    }

    pub fn analysis<F: Factory<Arc<dyn BrainAnalysis>>>(&mut self, f: F) -> &mut Self {
        self.analyses.push(Box::new(f));
        self
    }

    pub fn consolidator<F: Factory<Arc<dyn FactConsolidator>>>(&mut self, f: F) -> &mut Self {
        self.consolidators.push(Box::new(f));
        self
    }
}

impl fmt::Debug for BrainSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrainSlotBuilder")
            .field("filters", &self.filters.len())
            .field("extractors", &self.extractors.len())
            .field("validators", &self.validators.len())
            .field("importance", &self.importance.len())
            .field("sources", &self.sources.len())
            .field("retrievers", &self.retrievers.len())
            .field("rerankers", &self.rerankers.len())
            .field("injection", &self.injection.len())
            .field("outcome_signals", &self.outcome_signals.len())
            .field("analyses", &self.analyses.len())
            .field("consolidators", &self.consolidators.len())
            .finish()
    }
}

/// The built brain seams, in composition order.
#[derive(Clone, Default)]
pub struct BrainSeams {
    pub filters: Vec<Arc<dyn EventFilter>>,
    pub extractors: Vec<Arc<dyn FactExtractor>>,
    pub validators: Vec<Arc<dyn FactValidator>>,
    pub importance: Vec<Arc<dyn ImportanceModel>>,
    pub sources: Vec<Arc<dyn SourceContent>>,
    pub retrievers: Vec<Arc<dyn FactRetriever>>,
    pub rerankers: Vec<Arc<dyn Reranker>>,
    pub injection: Vec<Arc<dyn InjectionPolicy>>,
    pub outcome_signals: Vec<Arc<dyn OutcomeSignal>>,
    pub analyses: Vec<Arc<dyn BrainAnalysis>>,
    pub consolidators: Vec<Arc<dyn FactConsolidator>>,
    pub audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    pub source_access: Option<Arc<dyn SourceAccess>>,
}

impl BrainSeams {
    #[must_use]
    pub fn pipeline(&self) -> PipelineSeams {
        PipelineSeams {
            filters: self.filters.clone(),
            extractors: self.extractors.clone(),
            validators: self.validators.clone(),
            importance: self.importance.clone(),
            sources: self.sources.clone(),
        }
    }
}

impl fmt::Debug for BrainSeams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrainSeams")
            .field("filters", &self.filters.len())
            .field("extractors", &self.extractors.len())
            .field("validators", &self.validators.len())
            .field("importance", &self.importance.len())
            .field("sources", &self.sources.len())
            .field("retrievers", &self.retrievers.len())
            .field("rerankers", &self.rerankers.len())
            .field("injection", &self.injection.len())
            .field("outcome_signals", &self.outcome_signals.len())
            .field("analyses", &self.analyses.len())
            .field("consolidators", &self.consolidators.len())
            .field("audiences", &self.audiences.len())
            .field("source_access", &self.source_access.is_some())
            .finish()
    }
}

fn append_core_defaults(seams: &mut BrainSeams, r: &Resolved<'_>) {
    if let Some(content) = r.optional::<CoreSeam<dyn SourceContent, DocumentSourceContent>>() {
        seams.sources.push(content.seam());
    }
    if let Some(roles) = r.optional::<CoreSeam<dyn NotificationAudienceResolver, RoleAudience>>() {
        seams.audiences.push(roles.seam());
    }
    seams.source_access = r
        .optional::<CoreSeam<dyn SourceAccess, DocumentSourceAccess>>()
        .map(|access| access.seam());
}

impl Slot for BrainSlot {
    type Builder = BrainSlotBuilder;
    type Built = BrainSeams;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut seams = BrainSeams::default();
        for entry in entries {
            let owner = &entry.owner;
            let b = entry.builder;
            seams
                .filters
                .extend(build_factories(SLOT, owner, &b.filters, r)?);
            seams
                .extractors
                .extend(build_factories(SLOT, owner, &b.extractors, r)?);
            seams
                .validators
                .extend(build_factories(SLOT, owner, &b.validators, r)?);
            seams
                .importance
                .extend(build_factories(SLOT, owner, &b.importance, r)?);
            seams
                .sources
                .extend(build_factories(SLOT, owner, &b.sources, r)?);
            seams
                .retrievers
                .extend(build_factories(SLOT, owner, &b.retrievers, r)?);
            seams
                .rerankers
                .extend(build_factories(SLOT, owner, &b.rerankers, r)?);
            seams
                .injection
                .extend(build_factories(SLOT, owner, &b.injection, r)?);
            seams
                .outcome_signals
                .extend(build_factories(SLOT, owner, &b.outcome_signals, r)?);
            seams
                .analyses
                .extend(build_factories(SLOT, owner, &b.analyses, r)?);
            seams
                .consolidators
                .extend(build_factories(SLOT, owner, &b.consolidators, r)?);
        }
        append_core_defaults(&mut seams, r);
        Ok(seams)
    }
}
