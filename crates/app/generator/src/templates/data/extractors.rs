use anyhow::Result;
use serde_json::Value;
use systemprompt_content::models::ContentError;

pub fn extract_str_field<'a>(item: &'a Value, field: &str) -> Result<&'a str> {
    item.get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field(field).into())
}

pub fn extract_published_date(item: &Value) -> Result<&str> {
    item.get("published_at")
        .or_else(|| item.get("date"))
        .or_else(|| item.get("created_at"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("published_at/date/created_at").into())
}

pub fn extract_org_config(config: &serde_yaml::Value) -> Result<(&str, &str, &str)> {
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

pub fn extract_article_config(config: &serde_yaml::Value) -> Result<(&str, &str, &str)> {
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

pub fn extract_author<'a>(item: &'a Value, config: &'a serde_yaml::Value) -> Result<&'a str> {
    let default_author = config["metadata"]["default_author"]
        .as_str()
        .ok_or_else(|| ContentError::missing_field("metadata.default_author"))?;

    Ok(item
        .get("author")
        .and_then(|v| v.as_str())
        .filter(|a| !a.is_empty() && !a.contains("local"))
        .unwrap_or(default_author))
}

pub fn extract_twitter_handle(web_config: &serde_yaml::Value) -> Result<&str> {
    web_config
        .get("branding")
        .and_then(|b| b.get("twitter_handle"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_branding_config("twitter_handle").into())
}

pub fn extract_display_sitename(web_config: &serde_yaml::Value) -> Result<bool> {
    web_config
        .get("branding")
        .and_then(|b| b.get("display_sitename"))
        .and_then(serde_yaml::Value::as_bool)
        .ok_or_else(|| ContentError::missing_branding_config("display_sitename").into())
}

pub fn extract_logo_path(web_config: &serde_yaml::Value) -> Result<&str> {
    web_config
        .get("branding")
        .and_then(|b| b.get("logo"))
        .and_then(|l| l.get("primary"))
        .and_then(|p| p.get("svg"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_branding_config("branding.logo.primary.svg").into())
}

pub fn extract_favicon_path(web_config: &serde_yaml::Value) -> Result<&str> {
    web_config
        .get("branding")
        .and_then(|b| b.get("favicon"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_branding_config("branding.favicon").into())
}

pub fn format_date_pair(date_str: &str) -> (String, String) {
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
