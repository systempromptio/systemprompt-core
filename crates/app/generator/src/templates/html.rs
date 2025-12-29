use anyhow::{Context, Result};
use serde_json::Value;
use systemprompt_core_content::services::link::generation::GenerateContentLinkParams;
use systemprompt_core_content::services::LinkGenerationService;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::ContentId;

use systemprompt_core_content::models::ContentError;

use crate::content::{generate_related_card, CardData};

fn format_short_date(date_str: &str) -> String {
    if date_str.is_empty() {
        return String::new();
    }

    chrono::DateTime::parse_from_rfc3339(date_str).map_or_else(
        |_| date_str.to_string(),
        |dt| dt.format("%b %d, %Y").to_string(),
    )
}

fn extract_published_date(item: &Value) -> Result<&str> {
    item.get("published_at")
        .or_else(|| item.get("date"))
        .or_else(|| item.get("created_at"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("published_at/date/created_at").into())
}

async fn generate_section_cards(
    items: &[&Value],
    source_slug: &str,
    source_id: &ContentId,
    link_gen: &LinkGenerationService,
    section_prefix: &str,
) -> Result<Vec<String>> {
    let mut cards = Vec::new();

    for (index, rel_item) in items.iter().enumerate() {
        let title = rel_item
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("title"))?;
        let excerpt = rel_item
            .get("description")
            .or_else(|| rel_item.get("excerpt"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("description/excerpt"))?;
        let slug = rel_item
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("slug"))?;
        let image = rel_item.get("image").and_then(|v| v.as_str());

        let rel_date = extract_published_date(rel_item)?;
        let short_date = format_short_date(rel_date);

        // TODO: source_name should be passed in for proper URL generation
        let source_name = "blog"; // Temporary - should be extracted from item
        let target_url = format!("/{source_name}/{slug}");
        let source_page = format!("/{source_name}/{source_slug}");
        let link_position = format!("{}-{}", section_prefix, index + 1);

        let card_data = CardData {
            title,
            slug,
            description: excerpt,
            image,
            date: &short_date,
            url_prefix: &format!("/{source_name}"),
        };

        let content_id = ContentId::new(source_id.to_string());
        let card_url = match link_gen
            .generate_internal_content_link(GenerateContentLinkParams {
                target_url: &target_url,
                source_content_id: &content_id,
                source_page: &source_page,
                link_text: Some(title.to_string()),
                link_position: Some(link_position),
            })
            .await
        {
            Ok(tracked_link) => format!("/r/{}", tracked_link.short_code),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to generate tracked link, using direct URL");
                format!("/{source_name}/{slug}")
            },
        };

        cards.push(generate_related_card(&card_data, &card_url));
    }

    Ok(cards)
}

pub async fn generate_latest_and_popular_html(
    item: &Value,
    latest: &[&Value],
    popular: &[&Value],
    db_pool: DbPool,
) -> Result<String> {
    if latest.is_empty() && popular.is_empty() {
        return Ok(String::new());
    }

    let source_slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("slug"))?;
    let source_id = ContentId::new(
        item.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("id"))?,
    );

    let link_gen =
        LinkGenerationService::new(&db_pool).context("Failed to create link generation service")?;

    let latest_cards =
        generate_section_cards(latest, source_slug, &source_id, &link_gen, "latest").await?;
    let popular_cards =
        generate_section_cards(popular, source_slug, &source_id, &link_gen, "popular").await?;

    let mut sections = Vec::new();

    if !latest_cards.is_empty() {
        sections.push(format!(
            r#"<section class="related-section">
  <h3>Latest Posts</h3>
  <div class="related-grid">{}</div>
</section>"#,
            latest_cards.join("\n")
        ));
    }

    if !popular_cards.is_empty() {
        sections.push(format!(
            r#"<section class="related-section">
  <h3>Most Popular</h3>
  <div class="related-grid">{}</div>
</section>"#,
            popular_cards.join("\n")
        ));
    }

    if sections.is_empty() {
        return Ok(String::new());
    }

    Ok(format!(
        r#"<div class="related-articles">{}</div>"#,
        sections.join("\n")
    ))
}

pub async fn generate_cta_links(item: &Value, db_pool: DbPool) -> Result<(String, String)> {
    let source_slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ContentError::missing_field("slug"))?;
    let source_id = ContentId::new(
        item.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("id"))?,
    );

    let link_gen =
        LinkGenerationService::new(&db_pool).context("Failed to create link generation service")?;
    // TODO: source_name should be extracted from item for proper URL generation
    let source_name = item
        .get("source_id")
        .and_then(|v| v.as_str())
        .unwrap_or("blog");
    let source_page = format!("/{source_name}/{source_slug}");

    let header_cta_url = generate_single_cta(
        &link_gen,
        &source_id,
        &source_page,
        "Header Chat CTA",
        "header-cta",
    )
    .await;
    let banner_cta_url = generate_single_cta(
        &link_gen,
        &source_id,
        &source_page,
        "Banner Chat CTA",
        "banner-cta",
    )
    .await;

    Ok((header_cta_url, banner_cta_url))
}

async fn generate_single_cta(
    link_gen: &LinkGenerationService,
    source_id: &ContentId,
    source_page: &str,
    cta_name: &str,
    campaign: &str,
) -> String {
    match link_gen
        .generate_internal_content_link(GenerateContentLinkParams {
            target_url: "/",
            source_content_id: source_id,
            source_page,
            link_text: Some(cta_name.to_string()),
            link_position: Some(campaign.to_string()),
        })
        .await
    {
        Ok(tracked_link) => format!("/r/{}", tracked_link.short_code),
        Err(e) => {
            tracing::warn!(error = %e, cta = %cta_name, "Failed to generate CTA link");
            "/".to_string()
        },
    }
}

pub fn generate_references_html(item: &Value) -> Result<String> {
    let links_array = match item.get("links").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => return Ok(String::new()),
    };

    let mut cards = Vec::new();

    for (index, link) in links_array.iter().enumerate() {
        let title = link
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("link.title"))?;
        let url = link
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::missing_field("link.url"))?;
        let domain = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(ToString::to_string))
            .ok_or_else(|| ContentError::invalid_content(format!("Invalid URL: {}", url)))?;

        cards.push(format!(
            r#"<article class="reference-card">
  <span class="reference-card__number">[{}]</span>
  <a href="{}" class="reference-card__title" target="_blank" rel="noopener noreferrer">{}</a>
  <span class="reference-card__domain">{}</span>
</article>"#,
            index + 1,
            url,
            title,
            domain
        ));
    }

    if cards.is_empty() {
        return Ok(String::new());
    }

    Ok(format!(
        r#"<section class="references">
  <h2>References &amp; Sources</h2>
  <div class="references-grid">{}</div>
</section>"#,
        cards.join("\n")
    ))
}

pub async fn generate_social_content_html(_item: &Value, _db_pool: DbPool) -> Result<String> {
    Ok(String::new())
}
