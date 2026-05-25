//! Top-level prerender entry points: `prerender_content` walks every source
//! and renders content pages, while `prerender_pages` runs registered
//! page-prerenderer extensions to produce one-off pages (homepage, search,
//! error pages, …).

use std::collections::HashSet;
use std::path::PathBuf;

use systemprompt_database::DbPool;
use systemprompt_models::AppPaths;
use systemprompt_template_provider::{ComponentContext, PageContext, PagePrepareContext};
use tokio::fs;

use crate::error::{GeneratorResult as Result, PublishError};
use crate::prerender::content::process_all_sources;
use crate::prerender::context::{PrerenderContext, load_prerender_context};
use crate::prerender::utils::{merge_json_data, render_components};

pub async fn prerender_content(db_pool: DbPool, paths: &AppPaths) -> Result<()> {
    let ctx = load_prerender_context(db_pool, paths).await?;
    let total_rendered = process_all_sources(&ctx).await?;
    tracing::info!(items_rendered = total_rendered, "Prerendering completed");
    Ok(())
}

#[derive(Debug)]
pub struct PagePrerenderResult {
    pub page_type: String,
    pub output_path: PathBuf,
}

pub async fn prerender_pages(
    db_pool: DbPool,
    paths: &AppPaths,
) -> Result<Vec<PagePrerenderResult>> {
    let ctx = load_prerender_context(db_pool, paths).await?;
    prerender_pages_with_context(&ctx).await
}

async fn prerender_pages_with_context(ctx: &PrerenderContext) -> Result<Vec<PagePrerenderResult>> {
    let prerenderers = ctx.template_registry.page_prerenderers();

    if prerenderers.is_empty() {
        tracing::warn!("No page prerenderers registered - no pages will be rendered");
        return Ok(Vec::new());
    }

    let prerenderer_count = prerenderers.len();
    let page_types: Vec<_> = prerenderers.iter().map(|p| p.page_type()).collect();
    tracing::info!(
        count = prerenderer_count,
        page_types = ?page_types,
        "Discovered page prerenderers"
    );

    let mut results = Vec::new();

    for locale in &ctx.web_config.i18n.supported_locales {
        let locale_prefix = ctx.web_config.i18n.locale_prefix(locale);
        let prepare_ctx =
            PagePrepareContext::new(&ctx.web_config, &ctx.config, &ctx.db_pool, &ctx.dist_dir)
                .with_locale(locale);

        let mut rendered_page_types: HashSet<String> = HashSet::new();

        for prerenderer in prerenderers {
            let page_type = prerenderer.page_type();

            if rendered_page_types.contains(page_type) {
                tracing::debug!(
                    page_type = %page_type,
                    locale = %locale,
                    priority = prerenderer.priority(),
                    "Skipping prerenderer, page type already rendered by higher-priority prerenderer"
                );
                continue;
            }

            let render_spec = prerenderer
                .prepare(&prepare_ctx)
                .await
                .map_err(|e| PublishError::page_prerenderer_failed(page_type, e.to_string()))?;

            let Some(spec) = render_spec else {
                tracing::debug!(page_type = %page_type, locale = %locale, "Prerenderer returned None, skipping");
                continue;
            };

            if !ctx.template_registry.has_template(&spec.template_name) {
                tracing::warn!(
                    page_type = %page_type,
                    template = %spec.template_name,
                    "Template not found, skipping page"
                );
                continue;
            }

            let mut page_data = spec.base_data;
            if let Some(obj) = page_data.as_object_mut() {
                obj.insert(
                    "locale".to_string(),
                    serde_json::Value::String(locale.to_string()),
                );
            }

            let page_ctx = PageContext::new(page_type, &ctx.web_config, &ctx.config, &ctx.db_pool)
                .with_locale(locale);
            let providers = ctx.template_registry.page_providers_for(page_type);
            let provider_ids: Vec<_> = providers.iter().map(|p| p.provider_id()).collect();

            tracing::debug!(
                page_type = %page_type,
                locale = %locale,
                provider_count = providers.len(),
                provider_ids = ?provider_ids,
                "Collecting page data from providers"
            );

            for provider in &providers {
                let data = provider.provide_page_data(&page_ctx).await.map_err(|e| {
                    PublishError::provider_failed(provider.provider_id(), e.to_string())
                })?;
                merge_json_data(&mut page_data, &data);
            }

            let component_ctx = ComponentContext::for_page(&ctx.web_config);
            render_components(
                &ctx.template_registry,
                page_type,
                &component_ctx,
                &mut page_data,
            )
            .await;

            let html = ctx
                .template_registry
                .render(&spec.template_name, &page_data)
                .map_err(|e| {
                    PublishError::render_failed(&spec.template_name, None, e.to_string())
                })?;

            let prefixed_output = if locale_prefix.is_empty() {
                spec.output_path.clone()
            } else {
                PathBuf::from(locale_prefix.trim_start_matches('/')).join(&spec.output_path)
            };
            let output_path = ctx.dist_dir.join(&prefixed_output);

            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            fs::write(&output_path, html).await?;

            tracing::info!(
                page_type = %page_type,
                locale = %locale,
                path = %output_path.display(),
                "Generated page"
            );

            rendered_page_types.insert(page_type.to_string());

            results.push(PagePrerenderResult {
                page_type: page_type.to_string(),
                output_path,
            });
        }
    }

    Ok(results)
}
