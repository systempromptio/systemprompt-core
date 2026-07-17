//! [`SitemapProvider`] contract for emitting sitemap URL entries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::collections::HashMap;
use systemprompt_identifiers::{LocaleCode, SourceId};

use crate::error::ProviderResult;

#[derive(Debug)]
pub struct SitemapContext<'a> {
    pub base_url: &'a str,
    pub source_name: &'a str,
}

#[derive(Debug, Clone)]
pub struct SitemapAlternate {
    pub hreflang: LocaleCode,
    pub href: String,
}

#[derive(Debug, Clone)]
pub struct SitemapUrlEntry {
    pub loc: String,
    pub lastmod: String,
    pub changefreq: String,
    pub priority: f32,
    pub alternates: Vec<SitemapAlternate>,
}

#[derive(Debug, Clone)]
pub struct PlaceholderMapping {
    pub placeholder: String,
    pub field: String,
}

#[derive(Debug, Clone)]
pub struct SitemapSourceSpec {
    pub source_id: SourceId,
    pub url_pattern: String,
    pub placeholders: Vec<PlaceholderMapping>,
    pub priority: f32,
    pub changefreq: String,
}

// Why: provider is consumed as a trait object by the generator crate; an
// async fn in a bare trait is not dyn-compatible, so #[async_trait] is
// required.
#[async_trait]
pub trait SitemapProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn source_specs(&self) -> Vec<SitemapSourceSpec> {
        vec![]
    }

    fn static_urls(&self, _base_url: &str) -> Vec<SitemapUrlEntry> {
        vec![]
    }

    // JSON: `content` is a polymorphic per-source content object (markdown
    // frontmatter, blog row, etc.) consumed by the provider to fill
    // placeholders in the URL pattern. Defining a typed enum here would force
    // every consumer into a tagged union; trait boundary input.
    async fn resolve_placeholders(
        &self,
        ctx: &SitemapContext<'_>,
        content: &serde_json::Value,
        placeholders: &[PlaceholderMapping],
    ) -> ProviderResult<HashMap<String, String>>;

    fn priority(&self) -> u32 {
        100
    }
}
