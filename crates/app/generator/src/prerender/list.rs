use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_content::models::ContentError;
use systemprompt_database::DbPool;
use systemprompt_models::{ContentConfigRaw, FullWebConfig};
use systemprompt_template_provider::{ComponentContext, PageContext};

use crate::prerender::utils::{merge_json_data, render_components};
use systemprompt_templates::TemplateRegistry;
use tokio::fs;

use crate::content::{generate_content_card, generate_toc, render_markdown, CardData};
use crate::templates::navigation::generate_footer_html;

struct IndexContentData {
    title: String,
    description: String,
    content_html: String,
    toc_html: String,
}

fn render_index_content(item: &serde_json::Value) -> Result<IndexContentData> {
    let title = item
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Index content missing title"))?
        .to_string();

    let description = item
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Index content missing description"))?
        .to_string();

    let markdown_content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");

    let rendered_html = render_markdown(markdown_content);
    let toc_result = generate_toc(markdown_content, &rendered_html);

    Ok(IndexContentData {
        title,
        description,
        content_html: toc_result.content_html,
        toc_html: toc_result.toc_html,
    })
}

pub struct RenderListParams<'a> {
    pub items: &'a [serde_json::Value],
    pub config: &'a ContentConfigRaw,
    pub web_config: &'a FullWebConfig,
    pub list_config: &'a systemprompt_models::ParentRoute,
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
            .field("dist_dir", &self.dist_dir)
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

    let list_content_type = format!("{}-list", source_name);
    let template_name = template_registry
        .find_template_for_content_type(&list_content_type)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No template registered for content type: {}",
                list_content_type
            )
        })?;

    let index_data = match index_content {
        Some(item) => Some(render_index_content(item)?),
        None => None,
    };

    let mut posts_html = Vec::new();

    for item in items {
        let item_slug = item
            .get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        if item_slug.is_empty() {
            continue;
        }

        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("title"))
            .with_context(|| format!("Processing item '{}'", item_slug))?;
        let slug = item
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("slug"))?;
        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("description"))
            .with_context(|| format!("Processing item '{}'", item_slug))?;
        let image = item.get("image").and_then(|v| v.as_str());
        let date = item
            .get("published_at")
            .and_then(|v| v.as_str())
            .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
            .map(|dt| dt.format("%B %d, %Y").to_string())
            .ok_or_else(|| ContentError::missing_field("published_at"))
            .with_context(|| format!("Processing item '{}'", item_slug))?;

        posts_html.push(generate_content_card(&CardData {
            title,
            slug,
            description,
            image,
            date: &date,
            url_prefix: &list_config.url,
        }));
    }

    let footer_html = generate_footer_html(web_config)?;

    let mut list_data = serde_json::json!({
        "POSTS": posts_html.join("\n"),
        "ITEMS": posts_html.join("\n"),
        "FOOTER_NAV": footer_html,
    });

    if let Some(idx) = &index_data {
        if let Some(obj) = list_data.as_object_mut() {
            obj.insert("TITLE".into(), serde_json::Value::String(idx.title.clone()));
            obj.insert(
                "DESCRIPTION".into(),
                serde_json::Value::String(idx.description.clone()),
            );
            obj.insert(
                "CONTENT".into(),
                serde_json::Value::String(idx.content_html.clone()),
            );
            obj.insert(
                "TOC_HTML".into(),
                serde_json::Value::String(idx.toc_html.clone()),
            );
            obj.insert("HAS_INDEX_CONTENT".into(), serde_json::Value::Bool(true));
        }
    }

    let mut page_ctx =
        PageContext::new(&list_content_type, web_config, config, db_pool).with_all_items(items);
    if let Some(item) = index_content {
        page_ctx = page_ctx.with_content_item(item);
    }
    let providers = template_registry.page_providers_for(&list_content_type);

    for provider in &providers {
        let data = provider
            .provide_page_data(&page_ctx)
            .await
            .map_err(|e| anyhow::anyhow!("Provider {} failed: {}", provider.provider_id(), e))?;
        merge_json_data(&mut list_data, &data);
    }

    let component_ctx = ComponentContext::for_list(web_config, items);
    render_components(
        template_registry,
        &list_content_type,
        &component_ctx,
        &mut list_data,
    )
    .await;

    let list_html = template_registry
        .render(template_name, &list_data)
        .context("Failed to render list route")?;

    let list_dir = dist_dir.join(list_config.url.trim_start_matches('/'));
    fs::create_dir_all(&list_dir).await?;
    fs::write(list_dir.join("index.html"), &list_html).await?;

    tracing::debug!(path = %list_config.url, "Generated list route");
    Ok(())
}
