use super::html::{base_styles, html_escape, mcp_app_bridge_script, HtmlBuilder};
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, Copy, Default)]
pub struct TextRenderer;

impl TextRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_text(artifact: &Artifact) -> String {
        let mut text_parts = Vec::new();

        for part in &artifact.parts {
            if let Some(text) = part.as_text() {
                text_parts.push(text.to_string());
            }
        }

        text_parts.join("\n\n")
    }
}

#[async_trait]
impl UiRenderer for TextRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Text
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let text = Self::extract_text(artifact);
        let title = artifact.name.as_deref().unwrap_or("Text");

        let formatted_text = format_text_content(&text);

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    <div class="text-content" id="text-content">
        {text}
    </div>
    <div class="text-actions">
        <button class="copy-btn" id="copy-btn" title="Copy to clipboard">
            <span class="copy-icon">📋</span> Copy
        </button>
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
                .map(|d| format!(r#"<p class="mcp-app-description">{}</p>"#, html_escape(d)))
                .unwrap_or_default(),
            text = formatted_text,
        );

        let script = format!(
            "{bridge}\n{app}",
            bridge = mcp_app_bridge_script(),
            app = include_str!("assets/js/text.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(text_styles())
            .body(&body)
            .add_script(&script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

fn format_text_content(text: &str) -> String {
    let escaped = html_escape(text);
    escaped
        .lines()
        .map(|line| format!("<p>{}</p>", if line.is_empty() { "&nbsp;" } else { line }))
        .collect::<Vec<_>>()
        .join("\n")
}

const fn text_styles() -> &'static str {
    include_str!("assets/css/text.css")
}
