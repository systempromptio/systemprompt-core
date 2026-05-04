//! Top-level RSS feed generation: discovers feed providers, runs each, and
//! writes the resulting XML files to the build output directory.

use super::xml::{RssChannel, RssItem, build_rss_xml};
use std::path::Path;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::{AppPaths, Config};
use systemprompt_provider_contracts::{RssFeedContext, RssFeedProvider};
use tokio::fs;

use super::default_provider::DefaultRssFeedProvider;
use crate::error::{GeneratorResult as Result, PublishError};

#[derive(Debug, Clone)]
pub struct GeneratedFeed {
    pub filename: String,
    pub xml: String,
    pub item_count: usize,
}

pub async fn generate_feed(db_pool: DbPool, paths: &AppPaths) -> Result<()> {
    let provider = DefaultRssFeedProvider::new(Arc::clone(&db_pool), paths).await?;
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![Arc::new(provider)];
    let feeds = generate_feed_with_providers(&providers, db_pool).await?;

    let web_dir = paths.web().dist().to_path_buf();

    for feed in feeds {
        let feed_path = web_dir.join(&feed.filename);
        ensure_parent_exists(&feed_path).await?;
        fs::write(&feed_path, &feed.xml).await?;
        tracing::info!(
            path = %feed_path.display(),
            items = feed.item_count,
            "Generated RSS feed"
        );
    }

    Ok(())
}

pub async fn generate_feed_with_providers(
    providers: &[Arc<dyn RssFeedProvider>],
    _db_pool: DbPool,
) -> Result<Vec<GeneratedFeed>> {
    let global_config = Config::get().map_err(PublishError::other)?;
    let base_url = &global_config.api_external_url;

    let mut feeds = Vec::new();

    for provider in providers {
        for spec in provider.feed_specs() {
            let ctx = RssFeedContext {
                base_url,
                source_name: spec.source_id.as_str(),
            };

            let metadata = provider.feed_metadata(&ctx).await.map_err(|e| {
                PublishError::provider_failed(provider.provider_id(), e.to_string())
            })?;

            let items = provider
                .fetch_items(&ctx, spec.max_items)
                .await
                .map_err(|e| {
                    PublishError::provider_failed(provider.provider_id(), e.to_string())
                })?;

            let rss_items: Vec<RssItem> = items
                .into_iter()
                .map(|item| RssItem {
                    title: item.title,
                    link: item.link,
                    description: item.description,
                    pub_date: item.pub_date,
                    guid: item.guid,
                    author: item.author,
                })
                .collect();

            let channel = RssChannel {
                title: metadata.title,
                link: metadata.link,
                description: metadata.description,
                items: rss_items.clone(),
            };

            let xml = build_rss_xml(&channel);

            feeds.push(GeneratedFeed {
                filename: spec.output_filename,
                xml,
                item_count: rss_items.len(),
            });
        }
    }

    if feeds.is_empty() {
        return Err(PublishError::config(
            "No RSS feeds generated. Ensure at least one RssFeedProvider is registered.",
        ));
    }

    Ok(feeds)
}

async fn ensure_parent_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }
    }
    Ok(())
}
