use anyhow::{Context, Result};
use serde_json::{json, Value};
use systemprompt_core_database::DbPool;

use systemprompt_core_content::models::ContentError;

const SLUG_PLACEHOLDER: &str = "{slug}";

use super::html::{
    generate_cta_links, generate_latest_and_popular_html, generate_references_html,
    generate_social_content_html,
};
use super::items::{find_latest_items, find_popular_items};
use super::navigation::{generate_footer_html, generate_social_action_bar_html};
use super::paper::{
    calculate_read_time, generate_toc_html, parse_paper_metadata, render_paper_sections_html,
};
use crate::content::{get_absolute_image_url, normalize_image_url};

#[derive(Debug)]
pub struct TemplateDataParams<'a> {
    pub item: &'a Value,
    pub all_items: &'a [Value],
    pub popular_ids: &'a [String],
    pub config: &'a serde_yaml::Value,
    pub web_config: &'a serde_yaml::Value,
    pub content_html: &'a str,
    pub url_pattern: &'a str,
    pub db_pool: DbPool,
}

struct DateData {
    formatted: String,
    iso: String,
    modified_formatted: String,
    modified_iso: String,
}

struct ImageData {
    featured: String,
    absolute_url: String,
    hero: String,
    hero_alt: String,
}

struct ContentData {
    related_html: String,
    references_html: String,
    social_html: String,
    header_cta_url: String,
    banner_cta_url: String,
    toc_html: String,
    sections_html: String,
}

struct OrgConfig<'a> {
    name: &'a str,
    url: &'a str,
    logo: &'a str,
}

struct ArticleConfig<'a> {
    article_type: &'a str,
    section: &'a str,
    language: &'a str,
}

struct BrandingData<'a> {
    author: &'a str,
    twitter_handle: &'a str,
    display_sitename: bool,
}

struct BuildTemplateJsonParams<'a> {
    item: &'a Value,
    content_html: &'a str,
    slug: &'a str,
    canonical_path: &'a str,
    date_data: &'a DateData,
    image_data: &'a ImageData,
    content_data: &'a ContentData,
    navigation: &'a (String, String, String),
    org: OrgConfig<'a>,
    article: ArticleConfig<'a>,
    branding: BrandingData<'a>,
}

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
        },
    })
}

fn extract_str_field<'a>(item: &'a Value, field: &str) -> Result<&'a str> {
    item.get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field(field).into())
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

fn extract_author<'a>(item: &'a Value, config: &'a serde_yaml::Value) -> Result<&'a str> {
    let default_author = config["metadata"]["default_author"]
        .as_str()
        .ok_or_else(|| ContentError::missing_field("metadata.default_author"))?;

    Ok(item
        .get("author")
        .and_then(|v| v.as_str())
        .filter(|a| !a.is_empty() && !a.contains("local"))
        .unwrap_or(default_author))
}

fn extract_twitter_handle(web_config: &serde_yaml::Value) -> Result<&str> {
    web_config
        .get("branding")
        .and_then(|b| b.get("twitter_handle"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_branding_config("twitter_handle").into())
}

fn extract_display_sitename(web_config: &serde_yaml::Value) -> Result<bool> {
    web_config
        .get("branding")
        .and_then(|b| b.get("display_sitename"))
        .and_then(serde_yaml::Value::as_bool)
        .ok_or_else(|| ContentError::missing_branding_config("display_sitename").into())
}

fn build_template_json(params: BuildTemplateJsonParams<'_>) -> Result<Value> {
    let BuildTemplateJsonParams {
        item,
        content_html,
        slug,
        canonical_path,
        date_data,
        image_data,
        content_data,
        navigation,
        org,
        article,
        branding,
    } = params;

    let title = item
        .get("title")
        .ok_or_else(|| ContentError::missing_field("title"))?;
    let description = item
        .get("description")
        .or_else(|| item.get("excerpt"))
        .ok_or_else(|| ContentError::missing_field("description/excerpt"))?;
    let keywords = item
        .get("keywords")
        .or_else(|| item.get("tags"))
        .ok_or_else(|| ContentError::missing_field("keywords/tags"))?;
    let read_time = calculate_read_time(content_html);

    Ok(json!({
        "TITLE": title,
        "DESCRIPTION": description,
        "AUTHOR": branding.author,
        "DATE": date_data.formatted,
        "DATE_PUBLISHED": date_data.formatted,
        "DATE_MODIFIED": date_data.modified_formatted,
        "DATE_ISO": date_data.iso,
        "DATE_MODIFIED_ISO": date_data.modified_iso,
        "READ_TIME": read_time,
        "KEYWORDS": keywords,
        "IMAGE": image_data.absolute_url,
        "FEATURED_IMAGE": image_data.featured,
        "CONTENT": content_html,
        "SLUG": slug,
        "CANONICAL_PATH": canonical_path,
        "ORG_NAME": org.name,
        "ORG_URL": org.url,
        "ORG_LOGO": org.logo,
        "TWITTER_HANDLE": branding.twitter_handle,
        "ARTICLE_TYPE": article.article_type,
        "ARTICLE_SECTION": article.section,
        "ARTICLE_LANGUAGE": article.language,
        "HEADER_CTA_URL": content_data.header_cta_url,
        "BANNER_CTA_URL": content_data.banner_cta_url,
        "SOCIAL_ACTION_BAR": navigation.1,
        "SOCIAL_ACTION_BAR_HERO": navigation.2,
        "RELATED_CONTENT": content_data.related_html,
        "REFERENCES": content_data.references_html,
        "SOCIAL_CONTENT": content_data.social_html,
        "FOOTER_NAV": navigation.0,
        "DISPLAY_SITENAME": branding.display_sitename,
        "HERO_IMAGE": image_data.hero,
        "HERO_ALT": image_data.hero_alt,
        "TOC_HTML": content_data.toc_html,
        "SECTIONS_HTML": content_data.sections_html,
    }))
}

fn format_date_pair(date_str: &str) -> (String, String) {
    if date_str.is_empty() {
        return (String::new(), String::new());
    }

    chrono::DateTime::parse_from_rfc3339(date_str).map_or_else(
        |_| (date_str.to_string(), date_str.to_string()),
        |dt| {
            (
                dt.format("%B %d, %Y").to_string(),
                dt.format("%Y-%m-%d").to_string(),
            )
        },
    )
}

fn extract_published_date(item: &Value) -> Result<&str> {
    item.get("published_at")
        .or_else(|| item.get("date"))
        .or_else(|| item.get("created_at"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("published_at/date/created_at").into())
}

fn extract_org_config(config: &serde_yaml::Value) -> Result<(&str, &str, &str)> {
    let org = config
        .get("metadata")
        .and_then(|m| m.get("structured_data"))
        .and_then(|s| s.get("organization"))
        .ok_or_else(|| ContentError::missing_org_config("organization"))?;

    let org_name = org
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_org_config("organization.name"))?;
    let org_url = org
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_org_config("organization.url"))?;
    let org_logo = org
        .get("logo")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_org_config("organization.logo"))?;

    Ok((org_name, org_url, org_logo))
}

fn extract_article_config(config: &serde_yaml::Value) -> Result<(&str, &str, &str)> {
    let article = config
        .get("metadata")
        .and_then(|m| m.get("structured_data"))
        .and_then(|s| s.get("article"))
        .ok_or_else(|| ContentError::missing_article_config("article"))?;

    let article_type = article
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_article_config("article.type"))?;
    let article_section = article
        .get("article_section")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_article_config("article.article_section"))?;
    let article_language = article
        .get("language")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_article_config("article.language"))?;

    Ok((article_type, article_section, article_language))
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
