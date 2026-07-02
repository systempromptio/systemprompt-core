//! Default `SitemapProvider` that drives sitemap generation from the
//! crate-local `content.yaml` configuration.

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use systemprompt_models::{AppPaths, ContentConfigRaw};
use systemprompt_provider_contracts::{
    PlaceholderMapping, ProviderResult, SitemapContext, SitemapProvider, SitemapSourceSpec,
    SitemapUrlEntry,
};
use tokio::fs;

use crate::error::{GeneratorResult, PublishError};

#[derive(Debug)]
pub struct DefaultSitemapProvider {
    content_config: ContentConfigRaw,
}

impl DefaultSitemapProvider {
    pub async fn new(paths: &AppPaths) -> GeneratorResult<Self> {
        let content_config = load_content_config(paths).await?;
        Ok(Self { content_config })
    }

    #[must_use]
    pub const fn from_config(content_config: ContentConfigRaw) -> Self {
        Self { content_config }
    }
}

pub(super) async fn load_content_config(paths: &AppPaths) -> GeneratorResult<ContentConfigRaw> {
    let config_path = paths.system().content_config();

    let yaml_content = fs::read_to_string(&config_path).await.map_err(|source| {
        PublishError::ContentConfigRead {
            path: config_path.to_path_buf(),
            source,
        }
    })?;

    serde_yaml::from_str(&yaml_content).map_err(|source| PublishError::ContentConfigParse {
        path: config_path.to_path_buf(),
        source,
    })
}

#[async_trait]
impl SitemapProvider for DefaultSitemapProvider {
    fn provider_id(&self) -> &'static str {
        "default-sitemap"
    }

    fn source_specs(&self) -> Vec<SitemapSourceSpec> {
        self.content_config
            .content_sources
            .iter()
            .filter(|(_, source)| source.enabled)
            .filter_map(|(_, source)| {
                source.sitemap.as_ref().and_then(|sitemap| {
                    sitemap.enabled.then(|| SitemapSourceSpec {
                        source_id: source.source_id.clone(),
                        url_pattern: sitemap.url_pattern.clone(),
                        placeholders: vec![PlaceholderMapping {
                            placeholder: "{slug}".to_owned(),
                            field: "slug".to_owned(),
                        }],
                        priority: sitemap.priority,
                        changefreq: sitemap.changefreq.clone(),
                    })
                })
            })
            .collect()
    }

    fn static_urls(&self, base_url: &str) -> Vec<SitemapUrlEntry> {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        self.content_config
            .content_sources
            .iter()
            .filter(|(_, source)| source.enabled)
            .filter_map(|(_, source)| {
                source.sitemap.as_ref().and_then(|sitemap| {
                    sitemap.parent_route.as_ref().and_then(|parent| {
                        parent.enabled.then(|| SitemapUrlEntry {
                            loc: format!("{}{}", base_url, parent.url),
                            lastmod: today.clone(),
                            changefreq: parent.changefreq.clone(),
                            priority: parent.priority,
                            alternates: Vec::new(),
                        })
                    })
                })
            })
            .collect()
    }

    async fn resolve_placeholders(
        &self,
        _ctx: &SitemapContext<'_>,
        content: &serde_json::Value,
        placeholders: &[PlaceholderMapping],
    ) -> ProviderResult<HashMap<String, String>> {
        let mut resolved = HashMap::new();

        for mapping in placeholders {
            if let Some(value) = content.get(&mapping.field) {
                let string_value = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string().trim_matches('"').to_owned(),
                };
                resolved.insert(mapping.placeholder.clone(), string_value);
            }
        }

        Ok(resolved)
    }
}
