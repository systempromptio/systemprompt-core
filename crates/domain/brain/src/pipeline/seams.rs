//! [`PipelineSeams`]: the extension-contributed capture seams, each list
//! in ascending registration priority.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::ExtensionContext;
use systemprompt_extension::brain::{
    EventFilter, FactExtractor, FactValidator, ImportanceModel, SourceContent,
    discover_brain_extractors, discover_brain_filters, discover_brain_importance_models,
    discover_brain_source_contents, discover_brain_validators,
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

impl PipelineSeams {
    #[must_use]
    pub fn discover(ctx: &dyn ExtensionContext) -> Self {
        Self {
            filters: discover_brain_filters()
                .iter()
                .map(|r| (r.factory)(ctx))
                .collect(),
            extractors: discover_brain_extractors()
                .iter()
                .map(|r| (r.factory)(ctx))
                .collect(),
            validators: discover_brain_validators()
                .iter()
                .map(|r| (r.factory)(ctx))
                .collect(),
            importance: discover_brain_importance_models()
                .iter()
                .map(|r| (r.factory)(ctx))
                .collect(),
            sources: discover_brain_source_contents()
                .iter()
                .map(|r| (r.factory)(ctx))
                .collect(),
        }
    }
}
