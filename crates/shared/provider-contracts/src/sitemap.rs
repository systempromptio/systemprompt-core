use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use systemprompt_identifiers::SourceId;

#[derive(Debug)]
pub struct SitemapContext<'a> {
    pub base_url: &'a str,
    pub source_name: &'a str,
}

#[derive(Debug, Clone)]
pub struct SitemapUrlEntry {
    pub loc: String,
    pub lastmod: String,
    pub changefreq: String,
    pub priority: f32,
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

#[async_trait]
pub trait SitemapProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn source_specs(&self) -> Vec<SitemapSourceSpec> {
        vec![]
    }

    fn static_urls(&self, base_url: &str) -> Vec<SitemapUrlEntry> {
        let _ = base_url;
        vec![]
    }

    async fn resolve_placeholders(
        &self,
        ctx: &SitemapContext<'_>,
        content: &serde_json::Value,
        placeholders: &[PlaceholderMapping],
    ) -> Result<HashMap<String, String>>;

    fn priority(&self) -> u32 {
        100
    }
}
