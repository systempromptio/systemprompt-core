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
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::{
    ArtifactType, CardCta, CardSection, PresentationCardArtifact,
};

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
            theme = card.theme.class_suffix(),
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
                <h2 class="card-section-heading">{icon}{heading}</h2>
                <div class="card-section-content">{content}</div>
            </div>"#,
                // Why: The icon is decorative; without aria-hidden a screen reader
                // announces the emoji's name in the middle of the heading.
                icon = section.icon.as_ref().map_or_else(String::new, |i| format!(
                    r#"<span class="card-section-icon" aria-hidden="true">{}</span>"#,
                    html_escape(i)
                )),
                heading = html_escape(&section.heading),
                content = render_section_content(section),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_section_content(section: &CardSection) -> String {
    match &section.content {
        JsonValue::Array(items) if !items.is_empty() => {
            let lis = items
                .iter()
                .map(|item| format!("<li>{}</li>", html_escape(&scalar_text(item))))
                .collect::<Vec<_>>()
                .concat();
            format!(r#"<ul class="card-section-list">{lis}</ul>"#)
        },
        JsonValue::Object(map) if !map.is_empty() => {
            let rows = map
                .iter()
                .map(|(k, v)| {
                    format!(
                        "<dt>{}</dt><dd>{}</dd>",
                        html_escape(k),
                        html_escape(&scalar_text(v))
                    )
                })
                .collect::<Vec<_>>()
                .concat();
            format!(r#"<dl class="card-section-pairs">{rows}</dl>"#)
        },
        _ => render_multiline(&section.content_display()),
    }
}

fn scalar_text(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Null => String::new(),
        other => other.to_string(),
    }
}

fn render_ctas(ctas: &[CardCta]) -> String {
    if ctas.is_empty() {
        return String::new();
    }

    let buttons = ctas
        .iter()
        .map(|cta| {
            format!(
                r#"<button type="button" class="card-cta card-cta-{variant}" data-cta-id="{id}">{icon}{label}</button>"#,
                variant = cta.variant.class_suffix(),
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

    // Why: The live region sits with the buttons so the outcome of a click is
    // announced and visible in the same place the click happened.
    format!(
        r#"<footer class="card-ctas">{buttons}<p class="card-cta-status" id="card-cta-status" role="status" aria-live="polite"></p></footer>"#
    )
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
