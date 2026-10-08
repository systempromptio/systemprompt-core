//! List artifact renderer.
//!
//! [`ListRenderer`] renders a list [`Artifact`] into an HTML [`UiResource`],
//! coercing string or object list items (title, description, icon, link)
//! into ordered, unordered, or unstyled markup per the artifact's style hint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script, safe_url};
use super::typed::{lenient, lenient_vec};
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use serde::Deserialize;
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
                && let Some(items_value) = obj.get("items")
            {
                items.extend(
                    lenient_vec::<_, ListEntry>(items_value)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(ListItem::from_entry),
                );
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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ListEntry {
    Text(String),
    Fields(Box<ListItemSpec>),
}

#[derive(Debug, Deserialize)]
struct ListItemSpec {
    #[serde(default, deserialize_with = "lenient")]
    title: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    label: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    summary: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    category: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    description: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    subtitle: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    icon: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    link: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    url: Option<String>,
}

impl ListItem {
    fn from_entry(entry: ListEntry) -> Option<Self> {
        let spec = match entry {
            ListEntry::Text(title) => {
                return Some(Self {
                    title,
                    summary: None,
                    description: None,
                    category: None,
                    icon: None,
                    link: None,
                });
            },
            ListEntry::Fields(spec) => spec,
        };

        Some(Self {
            title: spec.title.or(spec.name).or(spec.label)?,
            summary: spec.summary.filter(|s| !s.is_empty()),
            category: spec.category.filter(|s| !s.is_empty()),
            description: spec.description.or(spec.subtitle),
            icon: spec.icon,
            link: spec.link.or(spec.url),
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
                safe_url(link).map_or_else(
                    || format!(r#"<span class="item-title">{}</span>"#, html_escape(&self.title)),
                    |href| {
                        format!(
                            r#"<a href="{href}" class="item-link" target="_blank" rel="noopener noreferrer">{}<span class="visually-hidden"> (opens in a new tab)</span></a>"#,
                            html_escape(&self.title)
                        )
                    },
                )
            },
        );

        let body_html = [self.summary.as_ref(), self.description.as_ref()]
            .into_iter()
            .flatten()
            .map(|t| format!(r#"<p class="item-description">{}</p>"#, html_escape(t)))
            .collect::<Vec<_>>()
            .concat();

        let category_html = self.category.as_ref().map_or_else(String::new, |c| {
            format!(r#"<span class="item-category">{}</span>"#, html_escape(c))
        });

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

impl UiRenderer for ListRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::List
    }

    fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
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
