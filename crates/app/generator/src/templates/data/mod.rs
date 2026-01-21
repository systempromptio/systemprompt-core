mod builders;
mod extractors;
mod types;

pub use types::TemplateDataParams;

use anyhow::{Context, Result};
use serde_json::Value;
use systemprompt_content::models::ContentError;
use systemprompt_database::DbPool;

use self::builders::build_template_json;
use self::extractors::{
    extract_article_config, extract_author, extract_display_sitename, extract_favicon_path,
    extract_logo_path, extract_org_config, extract_published_date, extract_str_field,
    extract_twitter_handle, format_date_pair,
};
use self::types::{
    ArticleConfig, BrandingData, BuildTemplateJsonParams, ContentData, DateData, ImageData,
    OrgConfig,
};
use super::html::{
    generate_cta_links, generate_latest_and_popular_html, generate_references_html,
    generate_social_content_html,
};
use super::items::{find_latest_items, find_popular_items};
use super::navigation::{generate_footer_html, generate_social_action_bar_html};
use super::paper::{generate_toc_html, parse_paper_metadata, render_paper_sections_html};
use crate::content::{get_absolute_image_url, normalize_image_url};

const SLUG_PLACEHOLDER: &str = "{slug}";

pub async fn prepare_template_data(params: TemplateDataParams<'_>) -> Result<Value> {
    let TemplateDataParams {
        item,
        all_items,
        popular_ids,
        config,
        web_config,
        content_html,
        url_pattern,
        db_pool,
    } = params;

    let slug = extract_str_field(item, "slug")?;
    let canonical_path = url_pattern.replace(SLUG_PLACEHOLDER, slug);

    let (org_name, org_url, org_logo) = extract_org_config(config)?;
    let (article_type, article_section, article_language) = extract_article_config(config)?;

    let date_data = prepare_date_data(item)?;
    let image_data = prepare_image_data(item, content_html, org_url)?;
    let content_data =
        prepare_content_data(item, all_items, popular_ids, content_html, &db_pool).await?;

    let navigation = prepare_navigation_html(web_config)?;
    let author = extract_author(item, config)?;
    let twitter_handle = extract_twitter_handle(web_config)?;
    let display_sitename = extract_display_sitename(web_config)?;
    let logo_path = extract_logo_path(web_config)?;
    let favicon_path = extract_favicon_path(web_config)?;

    build_template_json(BuildTemplateJsonParams {
        item,
        content_html,
        slug,
        canonical_path: &canonical_path,
        date_data: &date_data,
        image_data: &image_data,
        content_data: &content_data,
        navigation: &navigation,
        org: OrgConfig {
            name: org_name,
            url: org_url,
            logo: org_logo,
        },
        article: ArticleConfig {
            article_type,
            section: article_section,
            language: article_language,
        },
        branding: BrandingData {
            author,
            twitter_handle,
            display_sitename,
            logo_path,
            favicon_path,
        },
    })
}

fn prepare_date_data(item: &Value) -> Result<DateData> {
    let published_date = extract_published_date(item)?;
    let (formatted, iso) = format_date_pair(published_date);

    let (modified_formatted, modified_iso) = item
        .get("updated_at")
        .and_then(|v| v.as_str())
        .map_or_else(|| (formatted.clone(), iso.clone()), format_date_pair);

    Ok(DateData {
        formatted,
        iso,
        modified_formatted,
        modified_iso,
    })
}

fn prepare_image_data(item: &Value, content_html: &str, org_url: &str) -> Result<ImageData> {
    let raw_image = item
        .get("image")
        .or_else(|| item.get("cover_image"))
        .and_then(|v| v.as_str());

    let featured =
        normalize_image_url(raw_image).ok_or_else(|| ContentError::missing_field("image"))?;
    let absolute_url = get_absolute_image_url(raw_image, org_url)
        .ok_or_else(|| ContentError::missing_field("image"))?;

    let (hero, hero_alt, _, _) = prepare_paper_data(item, content_html, &absolute_url, org_url)?;

    Ok(ImageData {
        featured,
        absolute_url,
        hero,
        hero_alt,
    })
}

async fn prepare_content_data(
    item: &Value,
    all_items: &[Value],
    popular_ids: &[String],
    content_html: &str,
    db_pool: &DbPool,
) -> Result<ContentData> {
    let latest = find_latest_items(item, all_items, 6)?;
    let latest_slugs: Vec<&str> = latest
        .iter()
        .filter_map(|i| i.get("slug").and_then(|v| v.as_str()))
        .collect();
    let popular = find_popular_items(item, all_items, popular_ids, &latest_slugs, 6)?;

    let related_html =
        generate_latest_and_popular_html(item, &latest, &popular, std::sync::Arc::clone(db_pool))
            .await?;
    let (header_cta_url, banner_cta_url) =
        generate_cta_links(item, std::sync::Arc::clone(db_pool)).await?;
    let references_html = generate_references_html(item)?;
    let social_html = generate_social_content_html(item, std::sync::Arc::clone(db_pool)).await?;

    let raw_image = item.get("image").and_then(|v| v.as_str());
    let (_, _, toc_html, sections_html) =
        prepare_paper_data(item, content_html, raw_image.unwrap_or(""), "")?;

    Ok(ContentData {
        related_html,
        references_html,
        social_html,
        header_cta_url,
        banner_cta_url,
        toc_html,
        sections_html,
    })
}

fn prepare_navigation_html(web_config: &serde_yaml::Value) -> Result<(String, String, String)> {
    let footer = generate_footer_html(web_config)?;
    let social_bar = generate_social_action_bar_html(web_config, false)?;
    let social_bar_hero = generate_social_action_bar_html(web_config, true)?;
    Ok((footer, social_bar, social_bar_hero))
}

fn prepare_paper_data(
    item: &Value,
    content_html: &str,
    absolute_image_url: &str,
    org_url: &str,
) -> Result<(String, String, String, String)> {
    let content_type = item.get("content_type").and_then(|v| v.as_str());

    if content_type != Some("paper") {
        return Ok((
            absolute_image_url.to_string(),
            String::new(),
            String::new(),
            content_html.to_string(),
        ));
    }

    let markdown_content = item
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("content"))?;

    let paper_meta =
        parse_paper_metadata(markdown_content).context("Failed to parse paper metadata")?;

    let hero_img = paper_meta.hero_image.as_ref().map_or_else(
        || absolute_image_url.to_string(),
        |i| {
            if i.starts_with("http") || i.starts_with('/') {
                i.clone()
            } else {
                format!("/{i}")
            }
        },
    );

    let title = item
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("title"))?;

    let hero_alt_text = paper_meta
        .hero_alt
        .clone()
        .unwrap_or_else(|| title.to_string());

    let toc = if paper_meta.toc {
        generate_toc_html(&paper_meta)
    } else {
        String::new()
    };

    let sections = render_paper_sections_html(markdown_content, &paper_meta, org_url);

    Ok((hero_img, hero_alt_text, toc, sections))
}
