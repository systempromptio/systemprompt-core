//! Drives [`call_frontmatter_processors`] through the public ingest pipeline.
//!
//! The frontmatter-processor loop runs the processors the service is
//! constructed with, read from the built content hooks. This test fills a
//! `ContentSlot` builder the way a member's `contribute` does, with two
//! processors: one whose `applies_to_sources` excludes the ingest source
//! (exercising the skip arm) and one that fails (exercising the error-logging
//! arm), builds the slot, and creates a fresh content row to invoke the loop
//! end to end.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_content::models::IngestionSource;
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::{ContentSlot, ContentSlotBuilder, IngestionOptions, IngestionService};
use systemprompt_extension::{Resolved, Slot, SlotEntry};
use systemprompt_identifiers::{CategoryId, ExtensionId, SourceId};
use systemprompt_provider_contracts::{
    Dependencies, FrontmatterContext, FrontmatterProcessor, ProviderError, ProviderResult,
};
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};
use uuid::Uuid;

struct SkippedProcessor;

#[async_trait]
impl FrontmatterProcessor for SkippedProcessor {
    fn processor_id(&self) -> &'static str {
        "test-skipped-processor"
    }

    fn applies_to_sources(&self) -> Vec<String> {
        vec!["a-different-source".to_owned()]
    }

    async fn process_frontmatter(&self, _ctx: &FrontmatterContext<'_>) -> ProviderResult<()> {
        panic!("processor scoped to another source must not run");
    }
}

struct FailingProcessor;

#[async_trait]
impl FrontmatterProcessor for FailingProcessor {
    fn processor_id(&self) -> &'static str {
        "test-failing-processor"
    }

    async fn process_frontmatter(&self, ctx: &FrontmatterContext<'_>) -> ProviderResult<()> {
        assert!(!ctx.content_id().is_empty());
        Err(ProviderError::Internal("intentional test failure".into()))
    }
}

fn processors(pool: &systemprompt_database::DbPool) -> Vec<Arc<dyn FrontmatterProcessor>> {
    let mut builder = ContentSlotBuilder::default();
    builder
        .frontmatter_processor(|_: &Resolved<'_>| {
            Ok(Arc::new(SkippedProcessor) as Arc<dyn FrontmatterProcessor>)
        })
        .frontmatter_processor(|_: &Resolved<'_>| {
            Ok(Arc::new(FailingProcessor) as Arc<dyn FrontmatterProcessor>)
        });
    let handle: Arc<dyn DatabaseHandle> = Arc::<systemprompt_database::Database>::clone(pool);
    let kernel = Kernel::new(handle, BackgroundTasks::new(), Dependencies::default());
    let hooks = ContentSlot::build(
        vec![SlotEntry::install(
            ExtensionId::new("test-frontmatter"),
            builder,
        )],
        &kernel.resolved(),
    )
    .expect("the content slot builds");
    hooks.frontmatter_processors().to_vec()
}

#[tokio::test]
async fn ingest_runs_injected_frontmatter_processors() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;

    let dir = tempfile::tempdir().expect("tempdir");
    let slug = format!("fp-{}", Uuid::new_v4().simple());
    let md = format!(
        "---\ntitle: \"FP\"\nslug: \"{slug}\"\nauthor: \"A\"\npublished_at: \"2024-01-15\"\nkind: \"article\"\ndescription: \"d\"\n---\n\nbody\n"
    );
    std::fs::write(dir.path().join(format!("{slug}.md")), md).expect("write md");

    let source_id = SourceId::new(format!("fp-src-{}", Uuid::new_v4()));
    let category = CategoryId::new("docs");
    let source = IngestionSource::new(&source_id, "docs", &category);

    let service = IngestionService::new(&pool, ContentRepository::new(&pool), processors(&pool));
    let report = service
        .ingest_directory(
            dir.path(),
            &source,
            IngestionOptions::default().with_override(true),
        )
        .await
        .expect("ingest");

    // The processor failure is logged and swallowed, so ingestion still reports
    // the file as processed with no aggregated errors.
    assert_eq!(report.files_processed, 1, "{report:?}");
    assert!(
        report.errors.is_empty(),
        "processor errors must not fail ingest: {report:?}"
    );

    ContentRepository::new(&pool)
        .delete_by_source(&source_id)
        .await
        .expect("cleanup");
}
