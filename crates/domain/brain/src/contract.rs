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
//! The seams are the contributed ones only. The capture pipeline takes the
//! five capture seams ([`BrainSeams::pipeline`]) and the composition root
//! appends the core source reader; recall takes the four recall seams ahead
//! of its own; the analysis job runs the contributed analyses after the core
//! ones; the consolidation job runs the contributed consolidators. A factory
//! that fails, or reads a value nothing provides, refuses the build: a seam
//! is never silently left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_extension::brain::{FactConsolidator, Reranker};
use systemprompt_extension::{
    BrainAnalysis, EventFilter, FactExtractor, FactRetriever, FactValidator, Factory,
    FactoryError, ImportanceModel, InjectionPolicy, OutcomeSignal, Resolved, Slot,
    SlotEntry, SlotError, SlotOrder, SourceContent,
};
use systemprompt_identifiers::ExtensionId;

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
            .finish()
    }
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
            seams.filters.extend(run_all(&b.filters, owner, r)?);
            seams.extractors.extend(run_all(&b.extractors, owner, r)?);
            seams.validators.extend(run_all(&b.validators, owner, r)?);
            seams.importance.extend(run_all(&b.importance, owner, r)?);
            seams.sources.extend(run_all(&b.sources, owner, r)?);
            seams.retrievers.extend(run_all(&b.retrievers, owner, r)?);
            seams.rerankers.extend(run_all(&b.rerankers, owner, r)?);
            seams.injection.extend(run_all(&b.injection, owner, r)?);
            seams
                .outcome_signals
                .extend(run_all(&b.outcome_signals, owner, r)?);
            seams.analyses.extend(run_all(&b.analyses, owner, r)?);
            seams
                .consolidators
                .extend(run_all(&b.consolidators, owner, r)?);
        }
        Ok(seams)
    }
}

fn run_all<T: ?Sized + 'static>(
    factories: &Factories<T>,
    owner: &ExtensionId,
    r: &Resolved<'_>,
) -> Result<Vec<Arc<T>>, SlotError> {
    factories
        .iter()
        .map(|factory| {
            factory.build(r).map_err(|error| match error {
                FactoryError::Missing(missing) => SlotError::Missing(missing),
                FactoryError::Failed(source) => SlotError::Entry {
                    slot: SLOT,
                    owner: owner.clone(),
                    source,
                },
            })
        })
        .collect()
}
