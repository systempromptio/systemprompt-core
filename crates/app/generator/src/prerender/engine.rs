use std::path::PathBuf;

use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_template_provider::{ComponentContext, PageContext, PagePrepareContext};
use tokio::fs;

use crate::error::PublishError;
use crate::prerender::content::process_all_sources;
use crate::prerender::context::{load_prerender_context, PrerenderContext};
use crate::prerender::utils::merge_json_data;

pub async fn prerender_content(db_pool: DbPool) -> Result<()> {
    let ctx = load_prerender_context(db_pool).await?;
    let total_rendered = process_all_sources(&ctx).await?;
    tracing::info!(items_rendered = total_rendered, "Prerendering completed");
    Ok(())
}

#[derive(Debug)]
pub struct PagePrerenderResult {
    pub page_type: String,
    pub output_path: PathBuf,
}

pub async fn prerender_pages(db_pool: DbPool) -> Result<Vec<PagePrerenderResult>> {
    let ctx = load_prerender_context(db_pool).await?;
    prerender_pages_with_context(&ctx).await
}

pub async fn prerender_pages_with_context(
    ctx: &PrerenderContext,
) -> Result<Vec<PagePrerenderResult>> {
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

    let prepare_ctx =
        PagePrepareContext::new(&ctx.web_config, &ctx.config, &ctx.db_pool, &ctx.dist_dir);

    let mut results = Vec::new();

    for prerenderer in prerenderers {
        let page_type = prerenderer.page_type();

        let render_spec = prerenderer
            .prepare(&prepare_ctx)
            .await
            .map_err(|e| PublishError::page_prerenderer_failed(page_type, e.to_string()))?;

        let Some(spec) = render_spec else {
            tracing::debug!(page_type = %page_type, "Prerenderer returned None, skipping");
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

        let page_ctx = PageContext::new(page_type, &ctx.web_config, &ctx.db_pool);
        let providers = ctx.template_registry.page_providers_for(page_type);
        let provider_ids: Vec<_> = providers.iter().map(|p| p.provider_id()).collect();

        tracing::debug!(
            page_type = %page_type,
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
        for component in ctx.template_registry.components_for(page_type) {
            let rendered = component.render(&component_ctx).await.map_err(|e| {
                PublishError::provider_failed(component.component_id(), e.to_string())
            })?;

            if let Some(obj) = page_data.as_object_mut() {
                obj.insert(
                    rendered.variable_name,
                    serde_json::Value::String(rendered.html),
                );
            }
        }

        let html = ctx
            .template_registry
            .render(&spec.template_name, &page_data)
            .map_err(|e| PublishError::render_failed(&spec.template_name, None, e.to_string()))?;

        let output_path = ctx.dist_dir.join(&spec.output_path);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&output_path, html).await?;

        tracing::info!(
            page_type = %page_type,
            path = %output_path.display(),
            "Generated page"
        );

        results.push(PagePrerenderResult {
            page_type: page_type.to_string(),
            output_path,
        });
    }

    Ok(results)
}
