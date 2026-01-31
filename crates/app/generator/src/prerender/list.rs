use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_database::DbPool;
use systemprompt_models::{ContentConfigRaw, FullWebConfig, ParentRoute};
use systemprompt_template_provider::{ComponentContext, PageContext};
use systemprompt_templates::TemplateRegistry;
use tokio::fs;

use crate::prerender::utils::{merge_json_data, render_components};

pub struct RenderListParams<'a> {
    pub items: &'a [serde_json::Value],
    pub config: &'a ContentConfigRaw,
    pub web_config: &'a FullWebConfig,
    pub list_config: &'a ParentRoute,
    pub source_name: &'a str,
    pub template_registry: &'a TemplateRegistry,
    pub dist_dir: &'a Path,
    pub index_content: Option<&'a serde_json::Value>,
    pub db_pool: &'a DbPool,
}

impl std::fmt::Debug for RenderListParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderListParams")
            .field("source_name", &self.source_name)
            .field("items_count", &self.items.len())
            .field("has_index_content", &self.index_content.is_some())
            .finish_non_exhaustive()
    }
}

pub async fn render_list_route(params: RenderListParams<'_>) -> Result<()> {
    let RenderListParams {
        items,
        config,
        web_config,
        list_config,
        source_name,
        template_registry,
        dist_dir,
        index_content,
        db_pool,
    } = params;

    let list_content_type = format!("{source_name}-list");

    let template_name = template_registry
        .find_template_for_content_type(&list_content_type)
        .ok_or_else(|| anyhow::anyhow!("No template for: {list_content_type}"))?;

    let mut list_data = serde_json::json!({
        "HAS_INDEX_CONTENT": index_content.is_some(),
    });

    let mut page_ctx = PageContext::new(&list_content_type, web_config, config, db_pool)
        .with_all_items(items);

    if let Some(item) = index_content {
        page_ctx = page_ctx.with_content_item(item);
    }

    for provider in template_registry.page_providers_for(&list_content_type) {
        let data = provider
            .provide_page_data(&page_ctx)
            .await
            .map_err(|e| anyhow::anyhow!("Provider {} failed: {e}", provider.provider_id()))?;
        merge_json_data(&mut list_data, &data);
    }

    let component_ctx = ComponentContext::for_list(web_config, items);
    render_components(template_registry, &list_content_type, &component_ctx, &mut list_data).await;

    let list_html = template_registry
        .render(template_name, &list_data)
        .context("Failed to render list route")?;

    let list_dir = dist_dir.join(list_config.url.trim_start_matches('/'));
    fs::create_dir_all(&list_dir).await?;
    fs::write(list_dir.join("index.html"), &list_html).await?;

    tracing::debug!(path = %list_config.url, "Generated list route");
    Ok(())
}
