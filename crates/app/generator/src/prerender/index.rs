use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_content::models::ContentError;
use systemprompt_models::{ContentConfigRaw, FullWebConfig, SitemapConfig};
use systemprompt_templates::TemplateRegistry;
use tokio::fs;

use crate::content::{generate_content_card, CardData};

pub struct GenerateParentIndexParams<'a> {
    pub source_name: &'a str,
    pub sitemap_config: &'a SitemapConfig,
    pub items: &'a [serde_json::Value],
    pub config: &'a ContentConfigRaw,
    pub web_config: &'a FullWebConfig,
    pub template_registry: &'a TemplateRegistry,
    pub dist_dir: &'a Path,
}

impl std::fmt::Debug for GenerateParentIndexParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenerateParentIndexParams")
            .field("source_name", &self.source_name)
            .field("items_count", &self.items.len())
            .field("dist_dir", &self.dist_dir)
            .finish_non_exhaustive()
    }
}

pub async fn generate_parent_index(params: &GenerateParentIndexParams<'_>) -> Result<bool> {
    let GenerateParentIndexParams {
        source_name,
        sitemap_config,
        items,
        config,
        web_config,
        template_registry,
        dist_dir,
    } = params;
    let parent_config = match &sitemap_config.parent_route {
        Some(c) if c.enabled => c,
        _ => return Ok(false),
    };

    let list_content_type = format!("{}-list", source_name);
    let template_name = template_registry
        .find_template_for_content_type(&list_content_type)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No template registered for content type: {}",
                list_content_type
            )
        })?;

    let posts_html = build_posts_html(items, &parent_config.url)?;
    let parent_data = build_parent_template_data(&posts_html, config, web_config, source_name)?;

    let parent_html = template_registry
        .render(template_name, &parent_data)
        .context("Failed to render parent route")?;

    let parent_dir = dist_dir.join(parent_config.url.trim_start_matches('/'));
    fs::create_dir_all(&parent_dir).await?;
    fs::write(parent_dir.join("index.html"), &parent_html).await?;

    tracing::debug!(url = %parent_config.url, "Generated parent route");
    Ok(true)
}

fn build_posts_html(items: &[serde_json::Value], url_prefix: &str) -> Result<Vec<String>> {
    let mut posts = Vec::new();

    for item in items {
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("title"))?;
        let slug = item
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("slug"))?;
        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("description"))?;
        let image = item.get("image").and_then(|v| v.as_str());
        let date = item
            .get("published_at")
            .and_then(|v| v.as_str())
            .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
            .map(|dt| dt.format("%B %d, %Y").to_string())
            .ok_or_else(|| ContentError::missing_field("published_at"))?;

        posts.push(generate_content_card(&CardData {
            title,
            slug,
            description,
            image,
            date: &date,
            url_prefix,
        }));
    }

    Ok(posts)
}

fn build_parent_template_data(
    posts_html: &[String],
    config: &ContentConfigRaw,
    web_config: &FullWebConfig,
    source_name: &str,
) -> Result<serde_json::Value> {
    let footer_html = String::new();

    let org = &config.metadata.structured_data.organization;
    let branding = &web_config.branding;

    let source_config = config.content_sources.get(source_name);
    let source_branding = source_config.and_then(|s| s.branding.as_ref());

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

    let source_url = format!("{}/{}", org.url, source_name);
    let logo_path = branding.logo.primary.svg.as_deref().unwrap_or("");

    Ok(serde_json::json!({
        "POSTS": posts_html.join("\n"),
        "ITEMS": posts_html.join("\n"),
        "FOOTER_NAV": footer_html,
        "ORG_NAME": &org.name,
        "ORG_URL": &org.url,
        "ORG_LOGO": &org.logo,
        "BLOG_NAME": blog_name,
        "BLOG_DESCRIPTION": blog_description,
        "BLOG_IMAGE": blog_image,
        "BLOG_KEYWORDS": blog_keywords,
        "BLOG_URL": source_url,
        "BLOG_LANGUAGE": &config.metadata.language,
        "TWITTER_HANDLE": &branding.twitter_handle,
        "HEADER_CTA_URL": "/",
        "DISPLAY_SITENAME": branding.display_sitename,
        "LOGO_PATH": logo_path,
        "FAVICON_PATH": &branding.favicon,
    }))
}
