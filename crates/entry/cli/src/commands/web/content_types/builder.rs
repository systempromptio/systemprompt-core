//! Pure construction and validation for new content-type entries.
//!
//! The `create` command resolves its inputs (flags or prompts) and delegates
//! the config shaping here, keeping the YAML-bound structures testable.

use anyhow::{Result, anyhow};
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::content_config::{
    ContentConfigRaw, ContentSourceConfigRaw, IndexingConfig, SitemapConfig,
};

/// Fails when `category_id` is not declared in the config, listing the
/// declared categories in the error.
pub fn ensure_category_exists(config: &ContentConfigRaw, category_id: &str) -> Result<()> {
    if config.categories.contains_key(category_id) {
        return Ok(());
    }
    let available: Vec<&String> = config.categories.keys().collect();
    Err(anyhow!(
        "Category '{}' not found. Available categories: {:?}",
        category_id,
        available
    ))
}

/// Builds the database-fed sitemap entry used for flag-driven creation;
/// `None` when no URL pattern was supplied.
#[must_use]
pub fn build_sitemap_from_flags(
    url_pattern: Option<String>,
    priority: f32,
    changefreq: &str,
) -> Option<SitemapConfig> {
    url_pattern.map(|url_pattern| SitemapConfig {
        enabled: true,
        url_pattern,
        priority,
        changefreq: changefreq.to_owned(),
        fetch_from: "database".to_owned(),
        parent_route: None,
    })
}

/// Assembles a new article content source with the default indexing policy.
#[must_use]
pub fn build_source_config(
    path: String,
    source_id: &str,
    category_id: &str,
    enabled: bool,
    description: String,
    sitemap: Option<SitemapConfig>,
) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path,
        source_id: SourceId::new(source_id),
        category_id: CategoryId::new(category_id),
        enabled,
        description,
        allowed_content_types: vec!["article".to_owned()],
        indexing: Some(IndexingConfig {
            clear_before: false,
            recursive: true,
            override_existing: false,
        }),
        sitemap,
        branding: None,
    }
}
