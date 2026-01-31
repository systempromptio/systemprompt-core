use std::path::{Path, PathBuf};

use anyhow::Result;
use systemprompt_models::{ContentSourceConfigRaw, SitemapConfig};
use systemprompt_template_provider::{ComponentContext, ExtenderContext, PageContext};

use crate::prerender::utils::{merge_json_data, render_components};
use tokio::fs;

use crate::content::{generate_toc, render_markdown};
use crate::error::PublishError;
use crate::prerender::context::PrerenderContext;
use crate::prerender::fetch::{contents_to_json, fetch_content_for_source, fetch_popular_ids};
use crate::prerender::list::{render_list_route, RenderListParams};

const SLUG_PLACEHOLDER: &str = "{slug}";

pub async fn process_all_sources(ctx: &PrerenderContext) -> Result<u32> {
    let mut total_rendered = 0;

    for (source_name, source) in &ctx.config.content_sources {
        let Some(sitemap_config) = get_enabled_sitemap(source_name, source) else {
            continue;
        };

        let rendered = process_source(ctx, source_name, source, sitemap_config).await?;
        total_rendered += rendered;
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
    let contents = fetch_content_for_source(ctx, source_name, source.source_id.as_str())
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
    let popular_ids = fetch_popular_ids(ctx, source_name, source.source_id.as_str())
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
    let config_value = serde_yaml::to_value(&ctx.config)?;
    let mut rendered = 0;

    let parent_route_enabled = sitemap_config
        .parent_route
        .as_ref()
        .is_some_and(|p| p.enabled);

    for item in items {
        let slug = item.get("slug").and_then(|v| v.as_str()).unwrap_or("");
        if slug.is_empty() && parent_route_enabled {
            tracing::debug!(source = %source_name, "Skipping index content - rendered by parent route");
            continue;
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
        rendered += 1;
    }

    Ok(rendered)
}

struct RenderSingleItemParams<'a> {
    ctx: &'a PrerenderContext,
    source_name: &'a str,
    sitemap_config: &'a SitemapConfig,
    item: &'a serde_json::Value,
    all_items: &'a [serde_json::Value],
    popular_ids: &'a [String],
    config_value: &'a serde_yaml::Value,
}

async fn render_single_item(params: &RenderSingleItemParams<'_>) -> Result<()> {
    let RenderSingleItemParams {
        ctx,
        source_name,
        sitemap_config,
        item,
        all_items,
        popular_ids,
        config_value,
    } = params;

    let slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PublishError::missing_field("slug", "unknown"))?;

    let markdown_content = item
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PublishError::missing_field("content", slug))?;

    let rendered_html = render_markdown(markdown_content);
    let toc_result = generate_toc(markdown_content, &rendered_html);

    let content_type = item
        .get("content_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PublishError::missing_field("content_type", slug))?;

    let mut template_data = serde_json::json!({
        "CONTENT": toc_result.content_html,
        "TOC_HTML": toc_result.toc_html,
        "SLUG": slug,
    });

    let page_ctx = PageContext::new(content_type, &ctx.web_config, &ctx.config, &ctx.db_pool)
        .with_content_item(item)
        .with_all_items(all_items);

    for provider in ctx.template_registry.page_providers_for(content_type) {
        let data = provider
            .provide_page_data(&page_ctx)
            .await
            .map_err(|e| PublishError::provider_failed(provider.provider_id(), e.to_string()))?;
        merge_json_data(&mut template_data, &data);
    }

    let component_ctx =
        ComponentContext::for_content(&ctx.web_config, item, all_items, popular_ids);
    render_components(
        &ctx.template_registry,
        content_type,
        &component_ctx,
        &mut template_data,
    )
    .await;

    let extender_ctx =
        ExtenderContext::builder(item, all_items, config_value, &ctx.web_config, &ctx.db_pool)
            .with_content_html(&toc_result.content_html)
            .with_url_pattern(&sitemap_config.url_pattern)
            .with_source_name(source_name)
            .build();

    for extender in ctx.template_registry.extenders_for(content_type) {
        if let Err(e) = extender.extend(&extender_ctx, &mut template_data).await {
            tracing::warn!(
                extender_id = %extender.extender_id(),
                error = %e,
                "Template data extender failed"
            );
        }
    }

    let available_templates = ctx.template_registry.available_content_types();
    let template_name = ctx
        .template_registry
        .find_template_for_content_type(content_type)
        .ok_or_else(|| {
            PublishError::template_not_found(content_type, slug, available_templates.clone())
        })?;

    let html = ctx
        .template_registry
        .render(template_name, &template_data)
        .map_err(|e| {
            PublishError::render_failed(template_name, Some(slug.to_string()), e.to_string())
        })?;

    write_rendered_page(&ctx.dist_dir, &sitemap_config.url_pattern, slug, &html).await
}

async fn write_rendered_page(
    dist_dir: &Path,
    url_pattern: &str,
    slug: &str,
    html: &str,
) -> Result<()> {
    let output_dir = determine_output_dir(dist_dir, url_pattern, slug);
    fs::create_dir_all(&output_dir).await?;

    let output_path = output_dir.join("index.html");
    fs::write(&output_path, html).await?;

    let generated_path = url_pattern.replace(SLUG_PLACEHOLDER, slug);
    tracing::debug!(path = %generated_path, "Generated page");
    Ok(())
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

fn determine_output_dir(dist_dir: &Path, url_pattern: &str, slug: &str) -> PathBuf {
    let path = if slug.is_empty() {
        url_pattern
            .replace(&format!("/{SLUG_PLACEHOLDER}"), "")
            .replace(&format!("{SLUG_PLACEHOLDER}/"), "")
            .replace(SLUG_PLACEHOLDER, "")
    } else {
        url_pattern.replace(SLUG_PLACEHOLDER, slug)
    };
    match path.trim_start_matches('/') {
        "" => dist_dir.to_path_buf(),
        p => dist_dir.join(p),
    }
}
