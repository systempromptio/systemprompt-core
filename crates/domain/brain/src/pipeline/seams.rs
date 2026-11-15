//! [`PipelineSeams`]: the contributed capture seams, each list in
//! composition order, taken from the built
//! [`BrainSeams`](crate::contract::BrainSeams).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::brain::{
    EventFilter, FactExtractor, FactValidator, ImportanceModel, SourceContent,
};

/// The filters, extractors, validators, importance models and source
/// readers the pipeline composes.
#[derive(Clone, Default)]
pub struct PipelineSeams {
    pub filters: Vec<Arc<dyn EventFilter>>,
    pub extractors: Vec<Arc<dyn FactExtractor>>,
    pub validators: Vec<Arc<dyn FactValidator>>,
    pub importance: Vec<Arc<dyn ImportanceModel>>,
    pub sources: Vec<Arc<dyn SourceContent>>,
}

impl std::fmt::Debug for PipelineSeams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineSeams")
            .field("filters", &self.filters.len())
            .field("extractors", &self.extractors.len())
            .field("validators", &self.validators.len())
            .field("importance", &self.importance.len())
            .field("sources", &self.sources.len())
            .finish()
    }
}
