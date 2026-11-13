//! Dispatches parsed frontmatter to the frontmatter processors of the enabled
//! extensions, which the ingestion service is constructed with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::scanner::ParsedFrontmatter;
use systemprompt_provider_contracts::{Dependencies, FrontmatterContext, FrontmatterProcessor};

pub(super) async fn call_frontmatter_processors(
    processors: &[Arc<dyn FrontmatterProcessor>],
    dependencies: &Dependencies,
    content_id_str: &str,
    slug: &str,
    source_name: &str,
    parsed: &ParsedFrontmatter,
) {
    for processor in processors {
        let applies = processor.applies_to_sources();
        if !applies.is_empty() && !applies.contains(&source_name.to_owned()) {
            continue;
        }

        let ctx = FrontmatterContext::new(
            content_id_str,
            slug,
            source_name,
            &parsed.raw_yaml,
            dependencies,
        );

        if let Err(e) = processor.process_frontmatter(&ctx).await {
            tracing::warn!(
                processor = %processor.processor_id(),
                content_id = %content_id_str,
                error = %e,
                "Frontmatter processor failed"
            );
        }
    }
}
