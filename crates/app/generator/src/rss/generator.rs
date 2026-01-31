use super::xml::{build_rss_xml, RssChannel, RssItem};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::{AppPaths, Config};
use systemprompt_provider_contracts::{RssFeedContext, RssFeedProvider};
use tokio::fs;

use super::default_provider::DefaultRssFeedProvider;

#[derive(Debug, Clone)]
pub struct GeneratedFeed {
    pub filename: String,
    pub xml: String,
    pub item_count: usize,
}

pub async fn generate_feed(db_pool: DbPool) -> Result<()> {
    let provider = DefaultRssFeedProvider::new(Arc::clone(&db_pool)).await?;
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![Arc::new(provider)];
    let feeds = generate_feed_with_providers(&providers, db_pool).await?;

    let web_dir = AppPaths::get()
        .map_err(|e| anyhow!("{}", e))?
        .web()
        .dist()
        .to_path_buf();

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
    let global_config = Config::get()?;
    let base_url = &global_config.api_external_url;

    let mut feeds = Vec::new();

    for provider in providers {
        for spec in provider.feed_specs() {
            let ctx = RssFeedContext {
                base_url,
                source_name: spec.source_id.as_str(),
            };

            let metadata = provider
                .feed_metadata(&ctx)
                .await
                .context("Failed to fetch feed metadata")?;

            let items = provider
                .fetch_items(&ctx, spec.max_items)
                .await
                .context("Failed to fetch feed items")?;

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
        return Err(anyhow!(
            "No RSS feeds generated. Ensure at least one RssFeedProvider is registered."
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
