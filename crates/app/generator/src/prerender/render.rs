//! Per-item rendering: turns a single content row into rendered HTML on disk.
//!
//! Split out of [`crate::prerender::content`] to keep that module focused on
//! orchestration (source iteration, parallelism) rather than the rendering
//! pipeline of an individual item.

use std::path::{Path, PathBuf};

use systemprompt_identifiers::LocaleCode;
use systemprompt_models::SitemapConfig;
use systemprompt_template_provider::{ComponentContext, ExtenderContext, PageContext};
use tokio::fs;

use crate::content::render_markdown;
use crate::error::{GeneratorResult, PublishError};
use crate::prerender::context::PrerenderContext;
use crate::prerender::toc::generate_toc;
use crate::prerender::utils::{merge_json_data, render_components};

const SLUG_PLACEHOLDER: &str = "{slug}";

pub(super) struct RenderSingleItemParams<'a> {
    pub ctx: &'a PrerenderContext,
    pub source_name: &'a str,
    pub sitemap_config: &'a SitemapConfig,
    pub locale_prefix: &'a str,
    pub item: &'a serde_json::Value,
    pub all_items: &'a [serde_json::Value],
    pub popular_ids: &'a [String],
    pub config_value: &'a serde_yaml::Value,
}

pub(super) async fn render_single_item(params: &RenderSingleItemParams<'_>) -> GeneratorResult<()> {
    let RenderSingleItemParams {
        ctx,
        source_name,
        sitemap_config,
        locale_prefix,
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

    let locale = item
        .get("locale")
        .and_then(|v| v.as_str())
        .and_then(|s| LocaleCode::try_new(s).ok())
        .unwrap_or_else(|| ctx.web_config.i18n.default_locale.clone());

    let mut template_data = serde_json::json!({
        "CONTENT": toc_result.content_html,
        "TOC_HTML": toc_result.toc_html,
        "SLUG": slug,
        "locale": locale.as_str(),
    });

    let page_ctx = PageContext::new(content_type, &ctx.web_config, &ctx.config, &ctx.db_pool)
        .with_content_item(item)
        .with_all_items(all_items)
        .with_locale(&locale);

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

    write_rendered_page(
        &ctx.dist_dir,
        locale_prefix,
        &sitemap_config.url_pattern,
        slug,
        &html,
    )
    .await
}

async fn write_rendered_page(
    dist_dir: &Path,
    locale_prefix: &str,
    url_pattern: &str,
    slug: &str,
    html: &str,
) -> GeneratorResult<()> {
    let output_dir = determine_output_dir(dist_dir, locale_prefix, url_pattern, slug);
    fs::create_dir_all(&output_dir).await?;

    let output_path = output_dir.join("index.html");
    fs::write(&output_path, html).await?;

    let generated_path = format!(
        "{locale_prefix}{}",
        url_pattern.replace(SLUG_PLACEHOLDER, slug)
    );
    tracing::debug!(path = %generated_path, "Generated page");
    Ok(())
}

fn determine_output_dir(
    dist_dir: &Path,
    locale_prefix: &str,
    url_pattern: &str,
    slug: &str,
) -> PathBuf {
    let path = if slug.is_empty() {
        url_pattern
            .replace(&format!("/{SLUG_PLACEHOLDER}"), "")
            .replace(&format!("{SLUG_PLACEHOLDER}/"), "")
            .replace(SLUG_PLACEHOLDER, "")
    } else {
        url_pattern.replace(SLUG_PLACEHOLDER, slug)
    };
    let combined = format!("{locale_prefix}{path}");
    match combined.trim_start_matches('/') {
        "" => dist_dir.to_path_buf(),
        p => dist_dir.join(p),
    }
}
