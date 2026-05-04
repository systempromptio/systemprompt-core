//! [`SitemapProvider`] contract for emitting sitemap URL entries.

use async_trait::async_trait;
use std::collections::HashMap;
use systemprompt_identifiers::SourceId;

use crate::error::ProviderResult;

/// Per-call context for sitemap generation.
#[derive(Debug)]
pub struct SitemapContext<'a> {
    /// Site base URL used to absolutize relative paths.
    pub base_url: &'a str,
    /// Logical content source name driving this sitemap entry.
    pub source_name: &'a str,
}

/// One URL entry in a generated sitemap.
#[derive(Debug, Clone)]
pub struct SitemapUrlEntry {
    /// Absolute URL.
    pub loc: String,
    /// W3C-formatted last-modified timestamp.
    pub lastmod: String,
    /// Crawl-frequency hint (`daily`, `weekly`, ...).
    pub changefreq: String,
    /// Relative priority in `0.0..=1.0`.
    pub priority: f32,
}

/// Mapping from a URL placeholder to a JSON field on the content item.
#[derive(Debug, Clone)]
pub struct PlaceholderMapping {
    /// Placeholder name as it appears in [`SitemapSourceSpec::url_pattern`].
    pub placeholder: String,
    /// JSON field on the content item that fills the placeholder.
    pub field: String,
}

/// Static description of one source-driven sitemap section.
#[derive(Debug, Clone)]
pub struct SitemapSourceSpec {
    /// Content source feeding this sitemap section.
    pub source_id: SourceId,
    /// URL pattern with `{placeholder}` slots.
    pub url_pattern: String,
    /// Mappings used to substitute placeholders from content items.
    pub placeholders: Vec<PlaceholderMapping>,
    /// Default per-URL priority for this section.
    pub priority: f32,
    /// Default per-URL change-frequency hint for this section.
    pub changefreq: String,
}

/// Source-of-truth contract for sitemap generation.
///
/// Marked `#[async_trait]` because it is consumed via `dyn SitemapProvider`.
#[async_trait]
pub trait SitemapProvider: Send + Sync {
    /// Stable identifier for this provider.
    fn provider_id(&self) -> &'static str;

    /// Static list of source-driven sitemap sections this provider emits.
    fn source_specs(&self) -> Vec<SitemapSourceSpec> {
        vec![]
    }

    /// Static URL entries that do not flow from a content source.
    ///
    /// `_base_url` is supplied so implementations may absolutize relative
    /// paths; the default impl returns an empty vector and ignores it.
    fn static_urls(&self, _base_url: &str) -> Vec<SitemapUrlEntry> {
        vec![]
    }

    /// Resolve `placeholders` against `content` for a single sitemap entry.
    async fn resolve_placeholders(
        &self,
        ctx: &SitemapContext<'_>,
        content: &serde_json::Value,
        placeholders: &[PlaceholderMapping],
    ) -> ProviderResult<HashMap<String, String>>;

    /// Provider priority; higher values override lower ones.
    fn priority(&self) -> u32 {
        100
    }
}
