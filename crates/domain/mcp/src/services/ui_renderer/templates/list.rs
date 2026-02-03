use super::html::{base_styles, html_escape, mcp_app_bridge_script, HtmlBuilder};
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
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
            if let Some(data) = part.as_data() {
                if let Some(arr) = data.as_array() {
                    for item in arr {
                        if let Some(list_item) = ListItem::from_json(item) {
                            items.push(list_item);
                        }
                    }
                } else if let Some(obj) = data.as_object() {
                    if let Some(items_arr) = obj.get("items").and_then(JsonValue::as_array) {
                        for item in items_arr {
                            if let Some(list_item) = ListItem::from_json(item) {
                                items.push(list_item);
                            }
                        }
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
    description: Option<String>,
    icon: Option<String>,
    link: Option<String>,
}

impl ListItem {
    fn from_json(value: &JsonValue) -> Option<Self> {
        if let Some(s) = value.as_str() {
            return Some(Self {
                title: s.to_string(),
                description: None,
                icon: None,
                link: None,
            });
        }

        let title = value
            .get("title")
            .or_else(|| value.get("name"))
            .or_else(|| value.get("label"))
            .and_then(JsonValue::as_str)?
            .to_string();

        Some(Self {
            title,
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

    fn render_html(&self, index: usize) -> String {
        let icon_html = self.icon.as_ref().map_or_else(String::new, |i| {
            format!(r#"<span class="item-icon">{}</span>"#, html_escape(i))
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
                    r#"<a href="{}" class="item-link" target="_blank" rel="noopener">{}</a>"#,
                    html_escape(link),
                    html_escape(&self.title)
                )
            },
        );

        let description_html = self.description.as_ref().map_or_else(String::new, |d| {
            format!(r#"<p class="item-description">{}</p>"#, html_escape(d))
        });

        format!(
            r#"<li class="list-item" data-index="{index}">
    {icon}{title}
    {description}
</li>"#,
            index = index,
            icon = icon_html,
            title = title_html,
            description = description_html,
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

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let items = Self::extract_items(artifact);
        let style = Self::extract_list_style(artifact);
        let title = artifact.name.as_deref().unwrap_or("List");

        let items_html: String = items
            .iter()
            .enumerate()
            .map(|(i, item)| item.render_html(i))
            .collect();

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    <{tag} class="item-list {class}">
        {items}
    </{tag}>
    <div class="list-info">
        <span class="item-count">{count} items</span>
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
            tag = style.tag(),
            class = style.class(),
            items = items_html,
            count = items.len(),
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
