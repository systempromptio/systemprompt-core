//! Dispatches parsed frontmatter to extension-provided frontmatter processors.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::scanner::ParsedFrontmatter;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_provider_contracts::FrontmatterContext;

pub(super) async fn call_frontmatter_processors(
    db_pool: &DbPool,
    content_id_str: &str,
    slug: &str,
    source_name: &str,
    parsed: &ParsedFrontmatter,
) {
    let registry = ExtensionRegistry::discover().unwrap_or_else(|e| {
        tracing::error!(error = %e, "extension dependency cycle; using empty registry");
        ExtensionRegistry::new()
    });

    for ext in registry.extensions() {
        for processor in ext.frontmatter_processors() {
            let applies = processor.applies_to_sources();
            if !applies.is_empty() && !applies.contains(&source_name.to_owned()) {
                continue;
            }

            let ctx = FrontmatterContext::new(
                content_id_str,
                slug,
                source_name,
                &parsed.raw_yaml,
                db_pool,
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
}
