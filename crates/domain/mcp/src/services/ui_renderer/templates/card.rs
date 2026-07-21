//! Presentation-card artifact renderer.
//!
//! [`PresentationCardRenderer`] renders a [`PresentationCardArtifact`] into an
//! HTML [`UiResource`]: a titled card of heading/content sections, followed by
//! any call-to-action buttons, which forward their prompt to the host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{
    HtmlBuilder, base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script,
};
use super::typed::artifact_payload;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::{ArtifactType, CardCta, CardSection, PresentationCardArtifact};

#[derive(Debug, Clone, Copy, Default)]
pub struct PresentationCardRenderer;

impl PresentationCardRenderer {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UiRenderer for PresentationCardRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::PresentationCard
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let card: PresentationCardArtifact = artifact_payload(artifact)?;
        let title = if card.title.is_empty() {
            artifact.title.as_deref().unwrap_or("Card")
        } else {
            card.title.as_str()
        };

        let body = format!(
            r#"<div class="container">
    <section class="card card-theme-{theme}">
        <header class="card-header">
            <h1 class="mcp-app-title">{title}</h1>
            {subtitle_html}
        </header>
        <div class="card-sections">
            {sections_html}
        </div>
        {ctas_html}
    </section>
</div>"#,
            theme = html_escape(&card.theme),
            title = html_escape(title),
            subtitle_html = card.subtitle.as_ref().map_or_else(String::new, |s| format!(
                r#"<p class="mcp-app-description">{}</p>"#,
                html_escape(s)
            )),
            sections_html = render_sections(&card.sections),
            ctas_html = render_ctas(&card.ctas),
        );

        let script = format!(
            "{bridge}\nwindow.CARD_CTAS = {ctas};\n{app}",
            bridge = mcp_app_bridge_script(),
            ctas = json_to_js_literal(&serde_json::json!(&card.ctas)),
            app = include_str!("assets/js/card.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(card_styles())
            .body(&body)
            .add_script(&script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

fn render_sections(sections: &[CardSection]) -> String {
    if sections.is_empty() {
        return r#"<p class="card-empty">No details to display.</p>"#.to_owned();
    }

    sections
        .iter()
        .map(|section| {
            format!(
                r#"<div class="card-section">
                <div class="card-section-heading">{icon}{heading}</div>
                <div class="card-section-content">{content}</div>
            </div>"#,
                icon = section.icon.as_ref().map_or_else(String::new, |i| format!(
                    r#"<span class="card-section-icon">{}</span>"#,
                    html_escape(i)
                )),
                heading = html_escape(&section.heading),
                content = render_multiline(&section.content),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_ctas(ctas: &[CardCta]) -> String {
    if ctas.is_empty() {
        return String::new();
    }

    let buttons = ctas
        .iter()
        .map(|cta| {
            format!(
                r#"<button class="card-cta card-cta-{variant}" data-cta-id="{id}">{icon}{label}</button>"#,
                variant = html_escape(&cta.variant),
                id = html_escape(&cta.id),
                icon = cta.icon.as_ref().map_or_else(String::new, |i| format!(
                    r#"<span class="card-cta-icon">{}</span>"#,
                    html_escape(i)
                )),
                label = html_escape(&cta.label),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(r#"<footer class="card-ctas">{buttons}</footer>"#)
}

fn render_multiline(content: &str) -> String {
    html_escape(content)
        .lines()
        .map(|line| format!("<p>{}</p>", if line.is_empty() { "&nbsp;" } else { line }))
        .collect::<Vec<_>>()
        .join("\n")
}

const fn card_styles() -> &'static str {
    include_str!("assets/css/card.css")
}
