use anyhow::Result;
use serde_json::{json, Value};
use systemprompt_cloud::constants::storage;
use systemprompt_content::models::ContentError;

use super::types::BuildTemplateJsonParams;
use crate::templates::paper::calculate_read_time;

pub fn build_template_json(params: BuildTemplateJsonParams<'_>) -> Result<Value> {
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
        "LOGO_PATH": branding.logo_path,
        "FAVICON_PATH": branding.favicon_path,
        "HERO_IMAGE": image_data.hero,
        "HERO_ALT": image_data.hero_alt,
        "TOC_HTML": content_data.toc_html,
        "SECTIONS_HTML": content_data.sections_html,
        "CSS_BASE_PATH": format!("/{}", storage::CSS),
        "JS_BASE_PATH": format!("/{}", storage::JS),
    }))
}
