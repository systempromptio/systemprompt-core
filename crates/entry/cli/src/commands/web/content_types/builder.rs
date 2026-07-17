//! Pure construction and validation for new content-type entries.
//!
//! The `create` command resolves its inputs (flags or prompts) and delegates
//! the config shaping here, keeping the YAML-bound structures testable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

#[must_use]
pub fn build_flag_sitemap(url_pattern: String, priority: f32, changefreq: &str) -> SitemapConfig {
    SitemapConfig {
        enabled: true,
        url_pattern,
        priority,
        changefreq: changefreq.to_owned(),
        fetch_from: "database".to_owned(),
        parent_route: None,
    }
}

#[derive(Debug)]
pub struct SourceSpec {
    pub path: String,
    pub source_id: SourceId,
    pub category_id: CategoryId,
    pub enabled: bool,
    pub description: String,
    pub sitemap: Option<SitemapConfig>,
}

#[must_use]
pub fn build_source_config(spec: SourceSpec) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path: spec.path,
        source_id: spec.source_id,
        category_id: spec.category_id,
        enabled: spec.enabled,
        description: spec.description,
        allowed_content_types: vec!["article".to_owned()],
        indexing: Some(IndexingConfig {
            clear_before: false,
            recursive: true,
            override_existing: false,
        }),
        sitemap: spec.sitemap,
        branding: None,
    }
}
