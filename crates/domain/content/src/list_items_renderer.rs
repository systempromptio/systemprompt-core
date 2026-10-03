//! Component renderer emitting content-list item cards (with placeholder image
//! fallback).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ProviderResult, RenderedComponent,
};

const PLACEHOLDER_IMAGE_SVG: &str = r#"<div class="card-image card-image--placeholder">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
      <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
      <circle cx="8.5" cy="8.5" r="1.5"/>
      <polyline points="21 15 16 10 5 21"/>
    </svg>
  </div>"#;

#[derive(Debug, Clone, Copy, Default)]
pub struct ListItemsCardRenderer;

#[async_trait]
impl ComponentRenderer for ListItemsCardRenderer {
    fn component_id(&self) -> &'static str {
        "list-items-cards"
    }

    fn variable_name(&self) -> &'static str {
        "ITEMS"
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["blog-list".into(), "news-list".into(), "pages-list".into()]
    }

    async fn render(&self, ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent> {
        let items = ctx.all_items.unwrap_or(&[]);
        let url_prefix = extract_url_prefix(ctx);

        let cards_html: Vec<String> = items
            .iter()
            .filter_map(|item| render_card_html(item, &url_prefix))
            .collect();

        Ok(RenderedComponent::new(
            self.variable_name(),
            cards_html.join("\n"),
        ))
    }

    fn priority(&self) -> u32 {
        100
    }
}

#[derive(Debug, Deserialize)]
struct ListItemKind {
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct ListItemCard {
    title: String,
    slug: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
}

fn extract_url_prefix(ctx: &ComponentContext<'_>) -> String {
    ctx.all_items
        .and_then(|items| items.first())
        .and_then(|item| ListItemKind::deserialize(item).ok())
        .map_or_else(String::new, |kind| {
            let ct = kind.content_type.as_str();
            format!("/{}", ct.strip_suffix("-list").unwrap_or(ct))
        })
}

// JSON: Handlebars page context item; only the card fields are decoded.
fn render_card_html(item: &Value, url_prefix: &str) -> Option<String> {
    let card = ListItemCard::deserialize(item)
        .inspect_err(|e| tracing::debug!(error = %e, "Skipping list item without card fields"))
        .ok()?;
    let title = card.title.as_str();
    let slug = card.slug.as_str();
    let description = card.description.as_deref().unwrap_or("");
    let date = format_published_date(card.published_at.as_deref());

    let image_html = render_image_html(card.image.as_deref(), title);

    Some(format!(
        r#"<a href="{url_prefix}/{slug}" class="content-card-link">
  <article class="content-card">
    {image_html}
    <div class="card-content">
      <h2 class="card-title">{title}</h2>
      <p class="card-excerpt">{description}</p>
      <div class="card-meta">
        <time class="card-date">{date}</time>
      </div>
    </div>
  </article>
</a>"#
    ))
}

fn format_published_date(published_at: Option<&str>) -> String {
    published_at
        .and_then(|d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .inspect_err(|e| tracing::debug!(date = d, error = %e, "discarding unparseable published_at"))
                .ok()
        })
        .map_or_else(String::new, |dt| dt.format("%B %d, %Y").to_string())
}

fn render_image_html(image: Option<&str>, alt: &str) -> String {
    image.filter(|s| !s.is_empty()).map_or_else(
        || PLACEHOLDER_IMAGE_SVG.to_owned(),
        |img| {
            format!(
                r#"<div class="card-image">
    <img src="{img}" alt="{alt}" loading="lazy" />
  </div>"#
            )
        },
    )
}

pub fn default_list_items_renderer() -> Arc<dyn ComponentRenderer> {
    Arc::new(ListItemsCardRenderer)
}
