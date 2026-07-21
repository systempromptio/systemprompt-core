//! Plain-text and copy-paste-text artifact renderers.
//!
//! Both render the artifact's prose into an HTML [`UiResource`] with a
//! clipboard action. [`CopyPasteTextRenderer`] differs only in presenting the
//! content as a preformatted block, since its payload is meant to be copied
//! verbatim rather than read as prose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script};
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, Copy, Default)]
pub struct TextRenderer;

impl TextRenderer {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UiRenderer for TextRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Text
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        Ok(render_text(
            artifact,
            Presentation::Prose,
            self.csp_policy(),
        ))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CopyPasteTextRenderer;

impl CopyPasteTextRenderer {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UiRenderer for CopyPasteTextRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::CopyPasteText
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        Ok(render_text(
            artifact,
            Presentation::Preformatted,
            self.csp_policy(),
        ))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

#[derive(Debug, Clone, Copy)]
enum Presentation {
    Prose,
    Preformatted,
}

fn render_text(artifact: &Artifact, presentation: Presentation, csp: CspPolicy) -> UiResource {
    let text = extract_text(artifact);
    let title = payload_title(artifact)
        .or_else(|| artifact.title.clone())
        .unwrap_or_else(|| "Text".to_owned());

    let formatted_text = match presentation {
        Presentation::Prose => format_prose(&text),
        Presentation::Preformatted => format!("<pre><code>{}</code></pre>", html_escape(&text)),
    };

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
            format!(r#"<h1 class="mcp-app-title">{}</h1>"#, html_escape(&title))
        },
        description_html = artifact
            .description
            .as_ref()
            .map_or_else(String::new, |d| format!(
                r#"<p class="mcp-app-description">{}</p>"#,
                html_escape(d)
            )),
        text = formatted_text,
    );

    let script = format!(
        "{bridge}\n{app}",
        bridge = mcp_app_bridge_script(),
        app = include_str!("assets/js/text.js"),
    );

    let html = HtmlBuilder::new(&title)
        .add_style(base_styles())
        .add_style(text_styles())
        .body(&body)
        .add_script(&script)
        .build();

    UiResource::new(html).with_csp(csp)
}

fn extract_text(artifact: &Artifact) -> String {
    let mut text_parts = Vec::new();

    for part in &artifact.parts {
        if let Some(text) = part.as_text() {
            text_parts.push(text.to_owned());
        } else if let Some(content) = part
            .as_data()
            .and_then(|data| string_field(&data, "content"))
        {
            text_parts.push(content);
        }
    }

    text_parts.join("\n\n")
}

fn payload_title(artifact: &Artifact) -> Option<String> {
    artifact
        .parts
        .iter()
        .find_map(|part| part.as_data().and_then(|data| string_field(&data, "title")))
}

fn string_field(data: &serde_json::Value, field: &str) -> Option<String> {
    data.get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn format_prose(text: &str) -> String {
    html_escape(text)
        .lines()
        .map(|line| format!("<p>{}</p>", if line.is_empty() { "&nbsp;" } else { line }))
        .collect::<Vec<_>>()
        .join("\n")
}

const fn text_styles() -> &'static str {
    include_str!("assets/css/text.css")
}
