use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_content::models::ContentError;
use systemprompt_models::{ContentSourceConfigRaw, SitemapConfig};
use systemprompt_template_provider::{ComponentContext, ExtenderContext};
use tokio::fs;

use crate::content::render_markdown;
use crate::prerender::context::PrerenderContext;
use crate::prerender::fetch::{contents_to_json, fetch_content_for_source, fetch_popular_ids};
use crate::prerender::parent::{render_parent_route, RenderParentParams};
use crate::templates::data::{prepare_template_data, TemplateDataParams};

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
        .with_context(|| format!("Failed to fetch content for source '{}'", source_name))?;

    if contents.is_empty() {
        tracing::debug!(source = %source_name, "No content found for source");
        return Ok(0);
    }

    let items = contents_to_json(&contents);
    let popular_ids = fetch_popular_ids(ctx, source_name, source.source_id.as_str())
        .await
        .with_context(|| format!("Failed to fetch popular IDs for source '{}'", source_name))?;

    let rendered = render_all_items(ctx, source_name, sitemap_config, &items, &popular_ids).await?;
    let parent = render_parent_if_enabled(ctx, source_name, source, sitemap_config, &items).await?;
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

    for item in items {
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
        .ok_or_else(|| ContentError::missing_field("slug"))?;

    let markdown_content = item
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("content"))?;

    let content_html = render_markdown(markdown_content);

    let mut template_data = prepare_template_data(TemplateDataParams {
        item,
        all_items,
        popular_ids,
        config: config_value,
        web_config: &ctx.web_config,
        content_html: &content_html,
        url_pattern: &sitemap_config.url_pattern,
        db_pool: Arc::clone(&ctx.db_pool),
    })
    .await
    .with_context(|| format!("Failed to prepare template data for item '{}'", slug))?;

    let content_type = item
        .get("content_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("content_type"))?;

    let component_ctx =
        ComponentContext::for_content(&ctx.web_config, item, all_items, popular_ids);

    for component in ctx.template_registry.components_for(content_type) {
        match component.render(&component_ctx).await {
            Ok(rendered) => {
                if let Some(obj) = template_data.as_object_mut() {
                    obj.insert(
                        rendered.variable_name,
                        serde_json::Value::String(rendered.html),
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    component_id = %component.component_id(),
                    error = %e,
                    "Component render failed"
                );
            },
        }
    }

    let extender_ctx =
        ExtenderContext::builder(item, all_items, config_value, &ctx.web_config, &ctx.db_pool)
            .with_content_html(&content_html)
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

    let template_name = ctx
        .template_registry
        .find_template_for_content_type(content_type)
        .ok_or_else(|| {
            anyhow::anyhow!("No template registered for content type: {}", content_type)
        })?;

    let html = ctx
        .template_registry
        .render(template_name, &template_data)
        .with_context(|| format!("Failed to render template for item '{}'", slug))?;

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
    source: &ContentSourceConfigRaw,
    sitemap_config: &SitemapConfig,
    items: &[serde_json::Value],
) -> Result<u32> {
    let Some(parent_config) = &sitemap_config.parent_route else {
        return Ok(0);
    };

    if !parent_config.enabled {
        return Ok(0);
    }

    render_parent_route(RenderParentParams {
        items,
        config: &ctx.config,
        source,
        web_config: &ctx.web_config,
        parent_config,
        source_name,
        template_registry: &ctx.template_registry,
        dist_dir: &ctx.dist_dir,
    })
    .await?;

    Ok(1)
}

fn determine_output_dir(dist_dir: &Path, url_pattern: &str, slug: &str) -> PathBuf {
    let path = url_pattern.replace(SLUG_PLACEHOLDER, slug);
    let path = path.trim_start_matches('/');
    dist_dir.join(path)
}
