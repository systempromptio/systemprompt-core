use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_cloud::constants::storage;
use systemprompt_content::models::ContentError;
use systemprompt_database::DbPool;
use systemprompt_models::{ContentConfigRaw, ContentSourceConfigRaw, FullWebConfig};
use systemprompt_template_provider::{ComponentContext, PageContext};

use crate::prerender::utils::merge_json_data;
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

pub struct RenderParentParams<'a> {
    pub items: &'a [serde_json::Value],
    pub config: &'a ContentConfigRaw,
    pub source: &'a ContentSourceConfigRaw,
    pub web_config: &'a FullWebConfig,
    pub parent_config: &'a systemprompt_models::ParentRoute,
    pub source_name: &'a str,
    pub template_registry: &'a TemplateRegistry,
    pub dist_dir: &'a Path,
    pub index_content: Option<&'a serde_json::Value>,
    pub db_pool: &'a DbPool,
}

impl std::fmt::Debug for RenderParentParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderParentParams")
            .field("source_name", &self.source_name)
            .field("items_count", &self.items.len())
            .field("dist_dir", &self.dist_dir)
            .field("has_index_content", &self.index_content.is_some())
            .finish_non_exhaustive()
    }
}

pub async fn render_parent_route(params: RenderParentParams<'_>) -> Result<()> {
    let RenderParentParams {
        items,
        config,
        source,
        web_config,
        parent_config,
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
            url_prefix: &parent_config.url,
        }));
    }

    let footer_html = generate_footer_html(web_config)?;
    let org = &config.metadata.structured_data.organization;
    let source_branding = source.branding.as_ref();

    let mut parent_data = build_parent_data(&BuildParentDataParams {
        posts_html: &posts_html,
        footer_html: &footer_html,
        org,
        source_branding,
        web_config,
        language: &config.metadata.language,
        source_name,
        index_content: index_data.as_ref(),
    })?;

    let page_ctx = PageContext::new(&list_content_type, web_config, db_pool);
    let providers = template_registry.page_providers_for(&list_content_type);

    for provider in &providers {
        let data = provider.provide_page_data(&page_ctx).await.map_err(|e| {
            anyhow::anyhow!("Provider {} failed: {}", provider.provider_id(), e)
        })?;
        merge_json_data(&mut parent_data, &data);
    }

    let component_ctx = ComponentContext::for_list(web_config, items);

    for component in template_registry.components_for(&list_content_type) {
        match component.render(&component_ctx).await {
            Ok(rendered) => {
                if let Some(obj) = parent_data.as_object_mut() {
                    obj.insert(
                        rendered.variable_name.clone(),
                        serde_json::Value::String(rendered.html),
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    component_id = %component.component_id(),
                    error = %e,
                    "Parent route component render failed"
                );
            },
        }
    }

    let parent_html = template_registry
        .render(template_name, &parent_data)
        .context("Failed to render parent route")?;

    let parent_dir = dist_dir.join(parent_config.url.trim_start_matches('/'));
    fs::create_dir_all(&parent_dir).await?;
    fs::write(parent_dir.join("index.html"), &parent_html).await?;

    tracing::debug!(path = %parent_config.url, "Generated parent route");
    Ok(())
}

struct BuildParentDataParams<'a> {
    posts_html: &'a [String],
    footer_html: &'a str,
    org: &'a systemprompt_models::OrganizationData,
    source_branding: Option<&'a systemprompt_models::SourceBranding>,
    web_config: &'a FullWebConfig,
    language: &'a str,
    source_name: &'a str,
    index_content: Option<&'a IndexContentData>,
}

fn build_parent_data(params: &BuildParentDataParams<'_>) -> Result<serde_json::Value> {
    let BuildParentDataParams {
        posts_html,
        footer_html,
        org,
        source_branding,
        web_config,
        language,
        source_name,
        index_content,
    } = params;

    let branding = &web_config.branding;

    let blog_name = source_branding
        .and_then(|b| b.name.as_deref())
        .unwrap_or(&branding.name);

    let blog_description = source_branding
        .and_then(|b| b.description.as_deref())
        .unwrap_or(&branding.description);

    let blog_image = source_branding
        .and_then(|b| b.image.as_deref())
        .map(|img| format!("{}{img}", org.url))
        .ok_or_else(|| ContentError::missing_branding_config("image"))?;

    let blog_keywords = source_branding
        .and_then(|b| b.keywords.as_deref())
        .ok_or_else(|| ContentError::missing_branding_config("keywords"))?;

    let logo_path = branding.logo.primary.svg.as_deref().unwrap_or("");

    let (title, description, content_html, toc_html) = match index_content {
        Some(idx) => (
            idx.title.as_str(),
            idx.description.as_str(),
            idx.content_html.as_str(),
            idx.toc_html.as_str(),
        ),
        None => (blog_name, blog_description, "", ""),
    };

    Ok(serde_json::json!({
        "POSTS": posts_html.join("\n"),
        "ITEMS": posts_html.join("\n"),
        "FOOTER_NAV": footer_html,
        "ORG_NAME": org.name,
        "ORG_URL": org.url,
        "ORG_LOGO": org.logo,
        "BLOG_NAME": blog_name,
        "BLOG_DESCRIPTION": blog_description,
        "BLOG_IMAGE": blog_image,
        "BLOG_KEYWORDS": blog_keywords,
        "BLOG_URL": format!("{}/{}", org.url, source_name),
        "BLOG_LANGUAGE": language,
        "TWITTER_HANDLE": &branding.twitter_handle,
        "HEADER_CTA_URL": "/",
        "DISPLAY_SITENAME": branding.display_sitename,
        "LOGO_PATH": logo_path,
        "FAVICON_PATH": &branding.favicon,
        "CSS_BASE_PATH": format!("/{}", storage::CSS),
        "JS_BASE_PATH": format!("/{}", storage::JS),
        "TITLE": title,
        "DESCRIPTION": description,
        "CONTENT": content_html,
        "TOC_HTML": toc_html,
        "HAS_INDEX_CONTENT": index_content.is_some(),
    }))
}
