use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_provider_contracts::{ComponentContext, ComponentRenderer, RenderedComponent};

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
#[allow(clippy::unnecessary_literal_bound)]
impl ComponentRenderer for ListItemsCardRenderer {
    fn component_id(&self) -> &str {
        "list-items-cards"
    }

    fn variable_name(&self) -> &str {
        "ITEMS"
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["blog-list".into(), "news-list".into(), "pages-list".into()]
    }

    async fn render(&self, ctx: &ComponentContext<'_>) -> Result<RenderedComponent> {
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

fn extract_url_prefix(ctx: &ComponentContext<'_>) -> String {
    ctx.all_items
        .and_then(|items| items.first())
        .and_then(|item| item.get("content_type"))
        .and_then(Value::as_str)
        .map_or_else(String::new, |ct| {
            format!("/{}", ct.strip_suffix("-list").unwrap_or(ct))
        })
}

fn render_card_html(item: &Value, url_prefix: &str) -> Option<String> {
    let title = item.get("title")?.as_str()?;
    let slug = item.get("slug")?.as_str()?;
    let description = item
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    let image = item.get("image").and_then(Value::as_str);
    let date = format_published_date(item);

    let image_html = render_image_html(image, title);

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

fn format_published_date(item: &Value) -> String {
    item.get("published_at")
        .and_then(Value::as_str)
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
        .map_or_else(String::new, |dt| dt.format("%B %d, %Y").to_string())
}

fn render_image_html(image: Option<&str>, alt: &str) -> String {
    image.filter(|s| !s.is_empty()).map_or_else(
        || PLACEHOLDER_IMAGE_SVG.to_string(),
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
