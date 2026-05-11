//! Default `RssFeedProvider` that emits a feed for every enabled content
//! source, sourcing items directly from the content repository.

use async_trait::async_trait;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_models::{AppPaths, Config, ContentConfigRaw, WebConfig};
use systemprompt_provider_contracts::{
    ProviderError, ProviderResult, RssFeedContext, RssFeedItem, RssFeedMetadata, RssFeedProvider,
    RssFeedSpec,
};
use tokio::fs;

use crate::error::{GeneratorResult, PublishError};
use crate::templates::load_web_config;

const DEFAULT_MAX_ITEMS: i64 = 20;

pub struct DefaultRssFeedProvider {
    db_pool: DbPool,
    content_config: ContentConfigRaw,
    web_config: WebConfig,
}

impl std::fmt::Debug for DefaultRssFeedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultRssFeedProvider")
            .field(
                "content_sources",
                &self.content_config.content_sources.keys(),
            )
            .finish_non_exhaustive()
    }
}

impl DefaultRssFeedProvider {
    pub async fn new(db_pool: DbPool, paths: &AppPaths) -> GeneratorResult<Self> {
        let content_config = load_content_config(paths).await?;
        let web_config = load_web_config(paths)
            .await
            .map_err(|e| PublishError::other(format!("Failed to load web config: {e}")))?;

        Ok(Self {
            db_pool,
            content_config,
            web_config,
        })
    }

    fn get_source_branding(&self, source_name: &str) -> (String, String) {
        let default_title = &self.web_config.branding.title;
        let default_description = &self.web_config.branding.description;

        self.content_config
            .content_sources
            .get(source_name)
            .and_then(|source| source.branding.as_ref())
            .map_or_else(
                || (default_title.clone(), default_description.clone()),
                |branding| {
                    (
                        branding
                            .name
                            .clone()
                            .unwrap_or_else(|| default_title.clone()),
                        branding
                            .description
                            .clone()
                            .unwrap_or_else(|| default_description.clone()),
                    )
                },
            )
    }
}

async fn load_content_config(paths: &AppPaths) -> GeneratorResult<ContentConfigRaw> {
    let config_path = paths.system().content_config();

    let yaml_content = fs::read_to_string(&config_path)
        .await
        .map_err(|e| PublishError::other(format!("Failed to read content config: {e}")))?;

    serde_yaml::from_str(&yaml_content)
        .map_err(|e| PublishError::other(format!("Failed to parse content config: {e}")))
}

#[async_trait]
impl RssFeedProvider for DefaultRssFeedProvider {
    fn provider_id(&self) -> &'static str {
        "default-rss"
    }

    fn feed_specs(&self) -> Vec<RssFeedSpec> {
        self.content_config
            .content_sources
            .iter()
            .filter(|(_, source)| source.enabled)
            .filter(|(_, source)| source.sitemap.as_ref().is_some_and(|s| s.enabled))
            .map(|(name, source)| RssFeedSpec {
                source_id: source.source_id.clone(),
                max_items: DEFAULT_MAX_ITEMS,
                output_filename: format!("{}.xml", name),
            })
            .collect()
    }

    async fn feed_metadata(&self, ctx: &RssFeedContext<'_>) -> ProviderResult<RssFeedMetadata> {
        let (title, description) = self.get_source_branding(ctx.source_name);
        let global_config = Config::get().map_err(|e| {
            ProviderError::Configuration(format!("Failed to load global config: {e}"))
        })?;

        Ok(RssFeedMetadata {
            title,
            link: global_config.api_external_url.clone(),
            description,
            language: Some("en".to_string()),
        })
    }

    async fn fetch_items(
        &self,
        ctx: &RssFeedContext<'_>,
        limit: i64,
    ) -> ProviderResult<Vec<RssFeedItem>> {
        let source_config = self
            .content_config
            .content_sources
            .values()
            .find(|s| s.source_id.as_str() == ctx.source_name)
            .ok_or_else(|| {
                ProviderError::NotFound(format!("Source not found: {}", ctx.source_name))
            })?;

        let url_pattern = source_config
            .sitemap
            .as_ref()
            .map_or("/{slug}", |s| s.url_pattern.as_str());

        let repo = ContentRepository::new(&self.db_pool).map_err(|e| {
            ProviderError::Configuration(format!("Failed to create content repository: {e}"))
        })?;

        let source_id = SourceId::new(ctx.source_name);
        let content_items = repo
            .list_by_source_limited(&source_id, &LocaleCode::new("en"), limit)
            .await
            .map_err(|e| {
                ProviderError::RenderFailed(format!("Failed to fetch content for RSS feed: {e}"))
            })?;

        let items = content_items
            .into_iter()
            .map(|content| {
                let relative_url = url_pattern.replace("{slug}", &content.slug);
                let link = format!("{}{}", ctx.base_url, relative_url);
                RssFeedItem {
                    title: content.title,
                    link: link.clone(),
                    description: content.description,
                    pub_date: content.published_at,
                    guid: link,
                    author: Some(content.author),
                }
            })
            .collect();

        Ok(items)
    }
}
