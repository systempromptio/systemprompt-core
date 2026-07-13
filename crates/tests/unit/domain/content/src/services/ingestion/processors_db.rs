//! Drives [`call_frontmatter_processors`] through the public ingest pipeline.
//!
//! The frontmatter-processor loop only runs when the discovered extension
//! registry contains an extension that exposes processors. We inject one at
//! runtime (process-isolated per nextest test) carrying two processors: one
//! whose `applies_to_sources` excludes the ingest source (exercising the skip
//! arm) and one that fails (exercising the error-logging arm). Creating a fresh
//! content row then invokes the loop end to end.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_content::models::IngestionSource;
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::{IngestionOptions, IngestionService};
use systemprompt_extension::runtime_config::{InjectedExtensions, set_injected_extensions};
use systemprompt_extension::{Extension, ExtensionMetadata};
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_provider_contracts::{
    FrontmatterContext, FrontmatterProcessor, ProviderError, ProviderResult,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
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
        Err(ProviderError::Internal(
            "intentional test failure".to_owned(),
        ))
    }
}

struct ProcessorExtension;

impl Extension for ProcessorExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "test-frontmatter-ext",
            name: "Test Frontmatter Extension",
            version: "0.0.0",
        }
    }

    fn frontmatter_processors(&self) -> Vec<Arc<dyn FrontmatterProcessor>> {
        vec![Arc::new(SkippedProcessor), Arc::new(FailingProcessor)]
    }
}

#[tokio::test]
async fn ingest_runs_injected_frontmatter_processors() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");

    set_injected_extensions(InjectedExtensions {
        extensions: vec![Arc::new(ProcessorExtension)],
        ..Default::default()
    })
    .ok();

    let dir = tempfile::tempdir().expect("tempdir");
    let slug = format!("fp-{}", Uuid::new_v4().simple());
    let md = format!(
        "---\ntitle: \"FP\"\nslug: \"{slug}\"\nauthor: \"A\"\npublished_at: \"2024-01-15\"\nkind: \"article\"\ndescription: \"d\"\n---\n\nbody\n"
    );
    std::fs::write(dir.path().join(format!("{slug}.md")), md).expect("write md");

    let source_id = SourceId::new(format!("fp-src-{}", Uuid::new_v4()));
    let category = CategoryId::new("docs");
    let source = IngestionSource::new(&source_id, "docs", &category);

    let service = IngestionService::new(&pool).expect("service");
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
        .expect("repo")
        .delete_by_source(&source_id)
        .await
        .expect("cleanup");
}
