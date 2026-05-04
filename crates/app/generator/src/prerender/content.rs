//! Source-level orchestration for content prerendering: walks every enabled
//! content source, fetches and enriches its rows, and dispatches per-item
//! rendering (in [`crate::prerender::render`]) and parent / list rendering
//! (in [`crate::prerender::list`]).

use futures::stream::{self, StreamExt};
use systemprompt_models::{ContentSourceConfigRaw, SitemapConfig};

use crate::error::{GeneratorResult as Result, PublishError};
use crate::prerender::context::PrerenderContext;
use crate::prerender::fetch::{contents_to_json, fetch_content_for_source, fetch_popular_ids};
use crate::prerender::list::{RenderListParams, render_list_route};
use crate::prerender::render::{RenderSingleItemParams, render_single_item};

/// Iterate every enabled source in the configured content sources, prerender
/// each entry, and return the total page count.
pub async fn process_all_sources(ctx: &PrerenderContext) -> Result<u32> {
    const SOURCE_CONCURRENCY: usize = 2;

    let sources: Vec<_> = ctx
        .config
        .content_sources
        .iter()
        .filter_map(|(source_name, source)| {
            get_enabled_sitemap(source_name, source).map(|sitemap| (source_name, source, sitemap))
        })
        .collect();

    let futures: Vec<_> = sources
        .iter()
        .map(|&(source_name, source, sitemap_config)| {
            process_source(ctx, source_name, source, sitemap_config)
        })
        .collect();

    let results: Vec<Result<u32>> = stream::iter(futures)
        .buffer_unordered(SOURCE_CONCURRENCY)
        .collect()
        .await;

    let mut total_rendered = 0;
    for result in results {
        total_rendered += result?;
    }
    Ok(total_rendered)
}

fn get_enabled_sitemap<'a>(
    source_name: &str,
    source: &'a ContentSourceConfigRaw,
) -> Option<&'a SitemapConfig> {
    if !source.enabled {
        tracing::debug!(source = %source_name, "Skipping disabled source");
        return None;
    }

    source
        .sitemap
        .as_ref()
        .filter(|cfg| cfg.enabled)
        .or_else(|| {
            tracing::debug!(source = %source_name, "Skipping source with disabled sitemap");
            None
        })
}

async fn process_source(
    ctx: &PrerenderContext,
    source_name: &str,
    source: &ContentSourceConfigRaw,
    sitemap_config: &SitemapConfig,
) -> Result<u32> {
    let contents = fetch_content_for_source(ctx, source_name, &source.source_id)
        .await
        .map_err(|e| PublishError::fetch_failed(source_name, e.to_string()))?;

    if contents.is_empty() {
        tracing::debug!(source = %source_name, "No content found for source");
        return Ok(0);
    }

    let items = contents_to_json(
        &contents,
        source_name,
        &ctx.content_data_providers,
        &ctx.db_pool,
    )
    .await;
    let popular_ids = fetch_popular_ids(ctx, source_name, &source.source_id)
        .await
        .map_err(|e| PublishError::fetch_failed(source_name, e.to_string()))?;

    let rendered = render_all_items(ctx, source_name, sitemap_config, &items, &popular_ids).await?;
    let parent = render_parent_if_enabled(ctx, source_name, sitemap_config, &items).await?;
    Ok(rendered + parent)
}

async fn render_all_items(
    ctx: &PrerenderContext,
    source_name: &str,
    sitemap_config: &SitemapConfig,
    items: &[serde_json::Value],
    popular_ids: &[String],
) -> Result<u32> {
    const RENDER_CONCURRENCY: usize = 8;

    let config_value = serde_yaml::to_value(&ctx.config)?;

    let parent_route_enabled = sitemap_config
        .parent_route
        .as_ref()
        .is_some_and(|p| p.enabled);

    let futures: Vec<_> = items
        .iter()
        .map(|item| async {
            let slug = item.get("slug").and_then(|v| v.as_str()).unwrap_or("");
            if slug.is_empty() && parent_route_enabled {
                tracing::debug!(source = %source_name, "Skipping index content - rendered by parent route");
                return Ok(false);
            }

            render_single_item(&RenderSingleItemParams {
                ctx,
                source_name,
                sitemap_config,
                item,
                all_items: items,
                popular_ids,
                config_value: &config_value,
            })
            .await?;
            Ok(true)
        })
        .collect();

    let results: Vec<Result<bool>> = stream::iter(futures)
        .buffer_unordered(RENDER_CONCURRENCY)
        .collect()
        .await;

    let mut rendered = 0u32;
    for result in results {
        if result? {
            rendered += 1;
        }
    }
    Ok(rendered)
}

async fn render_parent_if_enabled(
    ctx: &PrerenderContext,
    source_name: &str,
    sitemap_config: &SitemapConfig,
    items: &[serde_json::Value],
) -> Result<u32> {
    let Some(parent_config) = &sitemap_config.parent_route else {
        return Ok(0);
    };

    if !parent_config.enabled {
        return Ok(0);
    }

    let index_content = items.iter().find(|item| {
        item.get("slug")
            .and_then(|v| v.as_str())
            .is_some_and(str::is_empty)
    });

    render_list_route(RenderListParams {
        items,
        config: &ctx.config,
        web_config: &ctx.web_config,
        list_config: parent_config,
        source_name,
        template_registry: &ctx.template_registry,
        dist_dir: &ctx.dist_dir,
        index_content,
        db_pool: &ctx.db_pool,
    })
    .await?;

    Ok(1)
}
