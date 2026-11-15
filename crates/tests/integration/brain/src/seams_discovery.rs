use std::sync::Arc;

use systemprompt_brain::BrainSeams;
use systemprompt_brain::defaults::FormulaImportance;
use systemprompt_brain::pipeline::PipelineSeams;
use systemprompt_extension::brain::{EventFilter, FilterVerdict, ImportanceModel};
use systemprompt_models::brain::BrainEnvelope;

struct NamedFilter(&'static str);

impl EventFilter for NamedFilter {
    fn name(&self) -> &'static str {
        self.0
    }

    fn decide(&self, _: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Ignore
    }
}

#[test]
fn the_pipeline_takes_the_built_capture_seams_in_order() {
    let built = BrainSeams {
        filters: vec![
            Arc::new(NamedFilter("seams-early")),
            Arc::new(NamedFilter("seams-late")),
        ],
        importance: vec![Arc::new(FormulaImportance)],
        ..BrainSeams::default()
    };

    let seams = built.pipeline();

    let filters: Vec<&str> = seams.filters.iter().map(|f| f.name()).collect();
    assert_eq!(filters, ["seams-early", "seams-late"]);
    let importance: Vec<&str> = seams.importance.iter().map(|m| m.name()).collect();
    assert_eq!(importance, vec![ImportanceModel::name(&FormulaImportance)]);
    assert!(seams.validators.is_empty());
    assert!(seams.extractors.is_empty());
    assert!(seams.sources.is_empty());

    let debug = format!("{seams:?}");
    assert!(debug.starts_with("PipelineSeams"), "{debug}");
    assert!(debug.contains("filters: 2"), "{debug}");
    assert!(debug.contains("importance: 1"), "{debug}");
    assert!(debug.contains("extractors: 0"), "{debug}");
    assert_eq!(
        format!("{:?}", PipelineSeams::default()),
        "PipelineSeams { filters: 0, extractors: 0, validators: 0, importance: 0, sources: 0 }"
    );
}

#[test]
fn the_built_seams_report_every_count() {
    let debug = format!("{:?}", BrainSeams::default());
    assert!(debug.starts_with("BrainSeams"), "{debug}");
    for field in [
        "filters: 0",
        "retrievers: 0",
        "rerankers: 0",
        "injection: 0",
        "outcome_signals: 0",
        "analyses: 0",
        "consolidators: 0",
    ] {
        assert!(debug.contains(field), "{debug}");
    }
}
