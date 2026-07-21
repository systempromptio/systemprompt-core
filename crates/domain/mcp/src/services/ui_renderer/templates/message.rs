//! Notice-message artifact renderer.
//!
//! [`MessageRenderer`] renders a [`MessageArtifact`] into an HTML
//! [`UiResource`]: one severity-styled line per [`NoticeLine`], which is what
//! CLI commands emit when they have status to report rather than data.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script};
use super::typed::artifact_payload;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::{ArtifactType, MessageArtifact, NoticeLine};

#[derive(Debug, Clone, Copy, Default)]
pub struct MessageRenderer;

impl MessageRenderer {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UiRenderer for MessageRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Message
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let message: MessageArtifact = artifact_payload(artifact)?;
        let title = artifact.title.as_deref().unwrap_or("Result");

        let body = format!(
            r#"<div class="container">
    <h1 class="mcp-app-title">{title}</h1>
    <ul class="notice-list">
        {lines_html}
    </ul>
</div>"#,
            title = html_escape(title),
            lines_html = render_lines(&message.messages),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(message_styles())
            .body(&body)
            .add_script(mcp_app_bridge_script())
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

fn render_lines(lines: &[NoticeLine]) -> String {
    if lines.is_empty() {
        return r#"<li class="notice notice-info">Command completed with no output.</li>"#.to_owned();
    }

    lines
        .iter()
        .map(|line| {
            format!(
                r#"<li class="notice notice-{level}"><span class="notice-marker"></span><span class="notice-text">{text}</span></li>"#,
                level = html_escape(&normalize_level(&line.level)),
                text = html_escape(&line.text),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_level(level: &str) -> String {
    match level.to_ascii_lowercase().as_str() {
        "success" | "ok" => "success".to_owned(),
        "warn" | "warning" => "warning".to_owned(),
        "error" | "err" | "fail" | "failure" => "error".to_owned(),
        _ => "info".to_owned(),
    }
}

const fn message_styles() -> &'static str {
    include_str!("assets/css/message.css")
}
