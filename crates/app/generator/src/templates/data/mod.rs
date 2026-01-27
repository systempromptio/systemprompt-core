mod builders;
mod extractors;
mod types;

pub use types::TemplateDataParams;

use anyhow::Result;
use serde_json::Value;
use systemprompt_database::DbPool;
use systemprompt_models::FullWebConfig;

use crate::error::PublishError;

use self::builders::build_template_json;
use self::extractors::{
    extract_article_config, extract_author, extract_org_config, extract_published_date,
    format_date_pair, get_display_sitename, get_favicon_path, get_logo_path, get_twitter_handle,
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
        slug,
    } = params;

    let canonical_path = url_pattern.replace(SLUG_PLACEHOLDER, slug);

    let (org_name, org_url, org_logo) =
        extract_org_config(config).map_err(|e| PublishError::missing_field(e.to_string(), slug))?;
    let (article_type, article_section, article_language) = extract_article_config(config)
        .map_err(|e| PublishError::missing_field(e.to_string(), slug))?;

    let date_data = prepare_date_data(item, slug)?;
    let image_data = prepare_image_data(item, org_url);
    let content_data =
        prepare_content_data(item, all_items, popular_ids, content_html, &db_pool).await?;

    let navigation = prepare_navigation_html(web_config)?;
    let author = extract_author(item, config)?;
    let twitter_handle = get_twitter_handle(web_config);
    let display_sitename = get_display_sitename(web_config);
    let logo_path = get_logo_path(web_config);
    let favicon_path = get_favicon_path(web_config);

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

fn prepare_date_data(item: &Value, slug: &str) -> Result<DateData> {
    let published_date = extract_published_date(item)
        .map_err(|e| PublishError::missing_field(e.to_string(), slug))?;
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

const DEFAULT_PLACEHOLDER_IMAGE: &str = "/files/images/placeholder.svg";

fn prepare_image_data(item: &Value, org_url: &str) -> ImageData {
    let raw_image = item
        .get("image")
        .or_else(|| item.get("cover_image"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let featured =
        normalize_image_url(raw_image).unwrap_or_else(|| DEFAULT_PLACEHOLDER_IMAGE.to_string());
    let absolute_url = get_absolute_image_url(raw_image, org_url).unwrap_or_else(|| {
        format!(
            "{}{}",
            org_url.trim_end_matches('/'),
            DEFAULT_PLACEHOLDER_IMAGE
        )
    });

    let title = item
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    ImageData {
        featured,
        absolute_url: absolute_url.clone(),
        hero: absolute_url,
        hero_alt: title.to_string(),
    }
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

    Ok(ContentData {
        related_html,
        references_html,
        social_html,
        header_cta_url,
        banner_cta_url,
        toc_html: String::new(),
        sections_html: content_html.to_string(),
    })
}

fn prepare_navigation_html(web_config: &FullWebConfig) -> Result<(String, String, String)> {
    let footer = generate_footer_html(web_config)?;
    let social_bar = generate_social_action_bar_html(web_config, false);
    let social_bar_hero = generate_social_action_bar_html(web_config, true);
    Ok((footer, social_bar, social_bar_hero))
}
