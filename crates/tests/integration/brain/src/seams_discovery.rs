use std::sync::Arc;

use systemprompt_brain::defaults::FormulaImportance;
use systemprompt_brain::pipeline::PipelineSeams;
use systemprompt_extension::brain::{EventFilter, FilterVerdict, ImportanceModel};
use systemprompt_extension::{register_brain_filter, register_brain_importance};
use systemprompt_models::brain::BrainEnvelope;
use systemprompt_test_fixtures::{DisposableDb, test_app_context};

struct NamedFilter(&'static str);

impl EventFilter for NamedFilter {
    fn name(&self) -> &'static str {
        self.0
    }

    fn decide(&self, _: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Ignore
    }
}

register_brain_filter!(
    factory = |_| Arc::new(NamedFilter("seams-late")) as Arc<dyn EventFilter>,
    priority = 40
);
register_brain_filter!(
    factory = |_| Arc::new(NamedFilter("seams-early")) as Arc<dyn EventFilter>,
    priority = -40
);
register_brain_importance!(FormulaImportance, priority = 0);

#[tokio::test]
async fn discovery_builds_each_registered_seam_in_priority_order() {
    let db = DisposableDb::with_schema("brain_seams_discover").await;
    let pool = db.test_pool().await;
    let ctx = test_app_context(&pool, db.url());

    let seams = PipelineSeams::discover(ctx.as_ref());

    let filters: Vec<&str> = seams.filters.iter().map(|f| f.name()).collect();
    let early = filters.iter().position(|n| *n == "seams-early");
    let late = filters.iter().position(|n| *n == "seams-late");
    assert!(early.is_some() && late.is_some(), "{filters:?}");
    assert!(early < late, "{filters:?}");
    let importance: Vec<&str> = seams.importance.iter().map(|m| m.name()).collect();
    assert_eq!(importance, vec![ImportanceModel::name(&FormulaImportance)]);
    assert!(seams.validators.is_empty());
    assert!(seams.extractors.is_empty());
    assert!(seams.sources.is_empty());

    let debug = format!("{seams:?}");
    assert!(debug.starts_with("PipelineSeams"), "{debug}");
    assert!(
        debug.contains(&format!("filters: {}", filters.len())),
        "{debug}"
    );
    assert!(debug.contains("importance: 1"), "{debug}");
    assert!(debug.contains("extractors: 0"), "{debug}");
    assert_eq!(
        format!("{:?}", PipelineSeams::default()),
        "PipelineSeams { filters: 0, extractors: 0, validators: 0, importance: 0, sources: 0 }"
    );

    pool.write_pool().close().await;
    db.drop_now().await;
}
