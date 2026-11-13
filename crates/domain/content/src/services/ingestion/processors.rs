//! Dispatches parsed frontmatter to the frontmatter processors of the enabled
//! extensions, which the ingestion service is constructed with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::{FrontmatterContext, FrontmatterProcessor};

pub(super) async fn call_frontmatter_processors(
    processors: &[Arc<dyn FrontmatterProcessor>],
    ctx: &FrontmatterContext<'_>,
) {
    for processor in processors {
        let applies = processor.applies_to_sources();
        if !applies.is_empty() && !applies.contains(&ctx.source_name().to_owned()) {
            continue;
        }

        if let Err(e) = processor.process_frontmatter(ctx).await {
            tracing::warn!(
                processor = %processor.processor_id(),
                content_id = %ctx.content_id(),
                error = %e,
                "Frontmatter processor failed"
            );
        }
    }
}
