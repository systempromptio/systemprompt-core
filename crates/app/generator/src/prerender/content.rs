//! Source-level orchestration for content prerendering: walks every enabled
//! content source, fetches and enriches its rows, and dispatches per-item
//! rendering (in [`crate::prerender::render`]) and parent / list rendering
//! (in [`crate::prerender::list`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::stream::{self, StreamExt};
use systemprompt_content::models::Content;
use systemprompt_identifiers::LocaleCode;
use systemprompt_models::{ContentSourceConfigRaw, SitemapConfig};

use crate::error::{GeneratorResult as Result, PublishError};
use crate::prerender::context::PrerenderContext;
use crate::prerender::fetch::{contents_to_json, fetch_content_for_source, fetch_popular_ids};
use crate::prerender::list::{RenderListParams, render_list_route};
use crate::prerender::render::{RenderSingleItemParams, remove_rendered_page, render_single_item};

struct SourceRenderJob<'a> {
    ctx: &'a PrerenderContext,
    source_name: &'a str,
    sitemap_config: &'a SitemapConfig,
    locale: &'a LocaleCode,
    locale_prefix: &'a str,
    items: &'a [serde_json::Value],
    popular_ids: &'a [String],
}

pub(super) async fn process_all_sources(ctx: &PrerenderContext) -> Result<u32> {
    const SOURCE_CONCURRENCY: usize = 2;

    let sources: Vec<_> = ctx
        .config
        .content_sources
        .iter()
        .filter_map(|(source_name, source)| {
            get_enabled_sitemap(source_name, source).map(|sitemap| (source_name, source, sitemap))
        })
        .collect();

    let locales = &ctx.web_config.i18n.supported_locales;

    let mut work = Vec::with_capacity(sources.len() * locales.len());
    for (source_name, source, sitemap) in &sources {
        for locale in locales {
            work.push((*source_name, *source, *sitemap, locale.clone()));
        }
    }

    let futures: Vec<_> = work
        .iter()
        .map(|(source_name, source, sitemap_config, locale)| {
            process_source(ctx, source_name, source, sitemap_config, locale)
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
    locale: &LocaleCode,
) -> Result<u32> {
    let contents = fetch_content_for_source(ctx, source_name, &source.source_id, locale)
        .await
        .map_err(|e| PublishError::fetch_failed(source_name, e.to_string()))?;

    if contents.is_empty() {
        tracing::debug!(source = %source_name, locale = %locale, "No content found for source/locale");
        return Ok(0);
    }

    let locale_prefix = ctx.web_config.i18n.locale_prefix(locale);
    let (public_contents, private_contents): (Vec<_>, Vec<_>) =
        contents.into_iter().partition(|c| c.public);

    purge_private_pages(ctx, sitemap_config, &locale_prefix, &private_contents).await;

    if public_contents.is_empty() {
        return Ok(0);
    }

    let items = contents_to_json(
        &public_contents,
        source_name,
        &ctx.content_data_providers,
        &ctx.db_pool,
    )
    .await;
    let popular_ids = fetch_popular_ids(ctx, source_name, &source.source_id)
        .await
        .map_err(|e| PublishError::fetch_failed(source_name, e.to_string()))?;

    let job = SourceRenderJob {
        ctx,
        source_name,
        sitemap_config,
        locale,
        locale_prefix: &locale_prefix,
        items: &items,
        popular_ids: &popular_ids,
    };

    let rendered = render_all_items(&job).await?;
    let parent = render_parent_if_enabled(&job).await?;
    Ok(rendered + parent)
}

async fn purge_private_pages(
    ctx: &PrerenderContext,
    sitemap_config: &SitemapConfig,
    locale_prefix: &str,
    private_contents: &[Content],
) {
    for content in private_contents {
        if content.slug.is_empty() {
            continue;
        }
        if let Err(e) = remove_rendered_page(
            &ctx.dist_dir,
            locale_prefix,
            &sitemap_config.url_pattern,
            &content.slug,
        )
        .await
        {
            tracing::warn!(slug = %content.slug, error = %e, "Failed to remove dist output for non-public slug");
        }
    }
}

async fn render_all_items(job: &SourceRenderJob<'_>) -> Result<u32> {
    const RENDER_CONCURRENCY: usize = 8;

    let config_value = serde_yaml::to_value(&job.ctx.config)?;

    let parent_route_enabled = job
        .sitemap_config
        .parent_route
        .as_ref()
        .is_some_and(|p| p.enabled);

    let futures: Vec<_> = job
        .items
        .iter()
        .map(|item| async {
            let slug = item.get("slug").and_then(|v| v.as_str()).unwrap_or("");
            if slug.is_empty() && parent_route_enabled {
                tracing::debug!(source = %job.source_name, "Skipping index content - rendered by parent route");
                return Ok(false);
            }

            render_single_item(&RenderSingleItemParams {
                ctx: job.ctx,
                source_name: job.source_name,
                sitemap_config: job.sitemap_config,
                locale_prefix: job.locale_prefix,
                item,
                all_items: job.items,
                popular_ids: job.popular_ids,
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

async fn render_parent_if_enabled(job: &SourceRenderJob<'_>) -> Result<u32> {
    let Some(parent_config) = &job.sitemap_config.parent_route else {
        return Ok(0);
    };

    if !parent_config.enabled {
        return Ok(0);
    }

    let index_content = job.items.iter().find(|item| {
        item.get("slug")
            .and_then(|v| v.as_str())
            .is_some_and(str::is_empty)
    });

    render_list_route(RenderListParams {
        items: job.items,
        config: &job.ctx.config,
        web_config: &job.ctx.web_config,
        list_config: parent_config,
        source_name: job.source_name,
        locale: job.locale,
        locale_prefix: job.locale_prefix,
        template_registry: &job.ctx.template_registry,
        dist_dir: &job.ctx.dist_dir,
        index_content,
        db_pool: &job.ctx.db_pool,
    })
    .await?;

    Ok(1)
}
