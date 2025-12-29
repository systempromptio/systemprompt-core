use anyhow::{Context, Result};
use std::path::Path;
use systemprompt_models::{ContentConfigRaw, SitemapConfig};
use tokio::fs;

use systemprompt_core_content::models::ContentError;

use crate::content::{generate_content_card, CardData};
use crate::templates::navigation::generate_footer_html;
use crate::templates::TemplateEngine;

#[derive(Debug)]
pub struct GenerateParentIndexParams<'a> {
    pub source_name: &'a str,
    pub sitemap_config: &'a SitemapConfig,
    pub items: &'a [serde_json::Value],
    pub config: &'a ContentConfigRaw,
    pub web_config: &'a serde_yaml::Value,
    pub templates: &'a TemplateEngine,
    pub dist_dir: &'a Path,
}

pub async fn generate_parent_index(params: &GenerateParentIndexParams<'_>) -> Result<bool> {
    let GenerateParentIndexParams {
        source_name,
        sitemap_config,
        items,
        config,
        web_config,
        templates,
        dist_dir,
    } = params;
    let parent_config = match &sitemap_config.parent_route {
        Some(c) if c.enabled => c,
        _ => return Ok(false),
    };

    // Use source-specific template if available, fall back to generic
    let template_name = match *source_name {
        "papers" => "paper-list",
        name => format!("{}-list", name).leak(), // e.g., "blog-list", "docs-list"
    };

    let posts_html = build_posts_html(items, &parent_config.url)?;
    let parent_data = build_parent_template_data(&posts_html, config, web_config, source_name)?;

    let parent_html = templates
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
    web_config: &serde_yaml::Value,
    source_name: &str,
) -> Result<serde_json::Value> {
    let footer_html = generate_footer_html(web_config)?;

    let org = &config.metadata.structured_data.organization;
    let org_name = &org.name;
    let org_url = &org.url;
    let org_logo = &org.logo;

    let source_config = config.content_sources.get(source_name);
    let source_branding = source_config.and_then(|s| s.branding.as_ref());

    let blog_name = source_branding
        .and_then(|b| b.name.as_deref())
        .or_else(|| {
            web_config
                .get("branding")
                .and_then(|b| b.get("name"))
                .and_then(|v| v.as_str())
        })
        .ok_or_else(|| ContentError::missing_branding_config("name"))?;

    let blog_description = source_branding
        .and_then(|b| b.description.as_deref())
        .or_else(|| {
            web_config
                .get("branding")
                .and_then(|b| b.get("description"))
                .and_then(|v| v.as_str())
        })
        .ok_or_else(|| ContentError::missing_branding_config("description"))?;

    let blog_image = source_branding
        .and_then(|b| b.image.as_deref())
        .map(|img| format!("{org_url}{img}"))
        .ok_or_else(|| ContentError::missing_branding_config("image"))?;

    let blog_keywords = source_branding
        .and_then(|b| b.keywords.as_deref())
        .ok_or_else(|| ContentError::missing_branding_config("keywords"))?;

    let source_url = format!("{}/{}", org_url, source_name);
    let blog_language = &config.metadata.language;

    let twitter_handle = web_config
        .get("branding")
        .and_then(|b| b.get("twitter_handle"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_branding_config("twitter_handle"))?;

    let display_sitename = web_config
        .get("branding")
        .and_then(|b| b.get("display_sitename"))
        .and_then(serde_yaml::Value::as_bool)
        .ok_or_else(|| ContentError::missing_branding_config("display_sitename"))?;

    Ok(serde_json::json!({
        "POSTS": posts_html.join("\n"),
        "ITEMS": posts_html.join("\n"),
        "FOOTER_NAV": footer_html,
        "ORG_NAME": org_name,
        "ORG_URL": org_url,
        "ORG_LOGO": org_logo,
        "BLOG_NAME": blog_name,
        "BLOG_DESCRIPTION": blog_description,
        "BLOG_IMAGE": blog_image,
        "BLOG_KEYWORDS": blog_keywords,
        "BLOG_URL": source_url,
        "BLOG_LANGUAGE": blog_language,
        "TWITTER_HANDLE": twitter_handle,
        "HEADER_CTA_URL": "/",
        "DISPLAY_SITENAME": display_sitename,
    }))
}
