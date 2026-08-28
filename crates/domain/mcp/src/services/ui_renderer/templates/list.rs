//! List artifact renderer.
//!
//! [`ListRenderer`] renders a list [`Artifact`] into an HTML [`UiResource`],
//! coercing string or object list items (title, description, icon, link)
//! into ordered, unordered, or unstyled markup per the artifact's style hint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script};
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, Copy, Default)]
pub struct ListRenderer;

impl ListRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_items(artifact: &Artifact) -> Vec<ListItem> {
        let mut items = Vec::new();

        for part in &artifact.parts {
            if let Some(data) = part.as_data()
                && let Some(obj) = data.as_object()
                && let Some(items_arr) = obj.get("items").and_then(JsonValue::as_array)
            {
                for item in items_arr {
                    if let Some(list_item) = ListItem::from_json(item) {
                        items.push(list_item);
                    }
                }
            }
        }

        items
    }

    fn extract_list_style(artifact: &Artifact) -> ListStyle {
        artifact
            .metadata
            .rendering_hints
            .as_ref()
            .and_then(|h| h.get("style"))
            .and_then(JsonValue::as_str)
            .map_or(ListStyle::Unordered, |s| match s {
                "ordered" | "numbered" => ListStyle::Ordered,
                "none" => ListStyle::None,
                _ => ListStyle::Unordered,
            })
    }
}

#[derive(Debug)]
struct ListItem {
    title: String,
    summary: Option<String>,
    description: Option<String>,
    category: Option<String>,
    icon: Option<String>,
    link: Option<String>,
}

impl ListItem {
    fn from_json(value: &JsonValue) -> Option<Self> {
        if let Some(s) = value.as_str() {
            return Some(Self {
                title: s.to_owned(),
                summary: None,
                description: None,
                category: None,
                icon: None,
                link: None,
            });
        }

        let title = value
            .get("title")
            .or_else(|| value.get("name"))
            .or_else(|| value.get("label"))
            .and_then(JsonValue::as_str)?
            .to_owned();

        Some(Self {
            title,
            summary: value
                .get("summary")
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from),
            category: value
                .get("category")
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from),
            description: value
                .get("description")
                .or_else(|| value.get("subtitle"))
                .and_then(JsonValue::as_str)
                .map(String::from),
            icon: value
                .get("icon")
                .and_then(JsonValue::as_str)
                .map(String::from),
            link: value
                .get("link")
                .or_else(|| value.get("url"))
                .and_then(JsonValue::as_str)
                .map(String::from),
        })
    }

    fn render_html(&self) -> String {
        let icon_html = self.icon.as_ref().map_or_else(String::new, |i| {
            format!(
                r#"<span class="item-icon" aria-hidden="true">{}</span>"#,
                html_escape(i)
            )
        });

        let title_html = self.link.as_ref().map_or_else(
            || {
                format!(
                    r#"<span class="item-title">{}</span>"#,
                    html_escape(&self.title)
                )
            },
            |link| {
                format!(
                    r#"<a href="{}" class="item-link" target="_blank" rel="noopener noreferrer">{}<span class="visually-hidden"> (opens in a new tab)</span></a>"#,
                    html_escape(link),
                    html_escape(&self.title)
                )
            },
        );

        // Why: Summary is the primary body text; description is secondary detail.
        // A payload carrying both shows both, in that order.
        let body_html = [self.summary.as_ref(), self.description.as_ref()]
            .into_iter()
            .flatten()
            .map(|t| format!(r#"<p class="item-description">{}</p>"#, html_escape(t)))
            .collect::<Vec<_>>()
            .concat();

        let category_html = self.category.as_ref().map_or_else(String::new, |c| {
            format!(r#"<span class="item-category">{}</span>"#, html_escape(c))
        });

        // Why: The row carried a hover treatment while only the inner anchor was
        // clickable, so clicking the row did nothing. `is-linked` stretches the
        // anchor over the whole row; a row with no link gets no affordance.
        // (`data-index` used to be emitted here for a list script that has
        // never existed.)
        format!(
            r#"<li class="list-item{linked}">
    {icon}{title}{category}
    {body}
</li>"#,
            linked = if self.link.is_some() {
                " is-linked"
            } else {
                ""
            },
            icon = icon_html,
            title = title_html,
            category = category_html,
            body = body_html,
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum ListStyle {
    Ordered,
    Unordered,
    None,
}

impl ListStyle {
    const fn tag(self) -> &'static str {
        match self {
            Self::Ordered => "ol",
            Self::Unordered | Self::None => "ul",
        }
    }

    const fn class(self) -> &'static str {
        match self {
            Self::Ordered => "ordered-list",
            Self::Unordered => "unordered-list",
            Self::None => "unstyled-list",
        }
    }
}

#[async_trait]
impl UiRenderer for ListRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::List
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let items = Self::extract_items(artifact);
        let style = Self::extract_list_style(artifact);
        let title = artifact.title.as_deref().unwrap_or("List");

        let items_html: String = items.iter().map(ListItem::render_html).collect();

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    {list_html}
    <div class="list-info">
        <span class="item-count">{count} {item_word}</span>
    </div>
</div>"#,
            title_html = if title.is_empty() {
                String::new()
            } else {
                format!(r#"<h1 class="mcp-app-title">{}</h1>"#, html_escape(title))
            },
            description_html = artifact
                .description
                .as_ref()
                .map_or_else(String::new, |d| format!(
                    r#"<p class="mcp-app-description">{}</p>"#,
                    html_escape(d)
                )),
            // Why: An empty <ul> rendered as nothing at all above a "0 items"
            // caption, which reads as a broken artifact rather than a result.
            list_html = if items.is_empty() {
                r#"<p class="list-empty">Nothing to show.</p>"#.to_owned()
            } else {
                format!(
                    r#"<{tag} class="item-list {class}">
        {items}
    </{tag}>"#,
                    tag = style.tag(),
                    class = style.class(),
                    items = items_html,
                )
            },
            count = items.len(),
            item_word = if items.len() == 1 { "item" } else { "items" },
        );

        let script = mcp_app_bridge_script();

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(list_styles())
            .body(&body)
            .add_script(script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

const fn list_styles() -> &'static str {
    include_str!("assets/css/list.css")
}
