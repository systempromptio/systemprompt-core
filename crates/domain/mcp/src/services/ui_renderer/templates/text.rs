//! Plain-text and copy-paste-text artifact renderers.
//!
//! Both render the artifact's prose into an HTML [`UiResource`] with a
//! clipboard action. [`CopyPasteTextRenderer`] differs only in presenting the
//! content as a preformatted block, since its payload is meant to be copied
//! verbatim rather than read as prose.
//!
//! # Blank lines and the markdown subset
//!
//! Prose was once rendered a line at a time, with a blank line becoming
//! `<p>&nbsp;</p>` — a full-height empty paragraph — while `.text-content` also
//! carried `white-space: pre-wrap`, so the newline between the tags rendered
//! again. One blank line in the source cost roughly two on screen, which is
//! what made a one-sentence tool result occupy a screenful. Blank lines are now
//! dropped and the separation comes from `p + p { margin-top }`, which is one
//! gap however many blank lines were typed.
//!
//! `format_prose` also renders three markdown constructs — `**bold**`,
//! `` `code` `` and `- ` bullets. Tools emit these constantly, and rendering
//! the markers literally is what put `- **[23] Follow up**` in front of a
//! reader. The subset stops there deliberately: anything richer is a markdown
//! renderer's job, not a plain-text artifact's.
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

    // Why: `.text-content-mono` existed in text.css from the start and was applied
    // by nothing, so the copy-paste variant rendered in the body font with the
    // browser's default monospace inside the <pre> — its whole intended
    // treatment was dead code.
    let language = payload_language(artifact);
    let (formatted_text, content_class) = match presentation {
        Presentation::Prose => (format_prose(&text), "text-content"),
        Presentation::Preformatted => (
            format!(
                "<pre><code{lang_class}>{body}</code></pre>",
                lang_class = language.as_ref().map_or_else(String::new, |l| format!(
                    r#" class="language-{}""#,
                    html_escape(l)
                )),
                body = html_escape(&text),
            ),
            "text-content text-content-mono",
        ),
    };

    let body = format!(
        r#"<div class="container">
    {title_html}
    {description_html}
    {language_html}
    <div class="{content_class}" id="text-content">
        {text}
    </div>
    <div class="text-actions">
        <button type="button" class="copy-btn" id="copy-btn">
            <span class="copy-icon" aria-hidden="true">📋</span> Copy
        </button>
        <span class="copy-status" id="copy-status" role="status" aria-live="polite"></span>
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
        language_html = language.as_ref().map_or_else(String::new, |l| format!(
            r#"<p class="text-language">{}</p>"#,
            html_escape(l)
        )),
        content_class = content_class,
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
    let escaped = html_escape(text);
    let mut out = String::with_capacity(escaped.len());
    let mut list_open = false;

    for line in escaped.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(item) = bullet_body(trimmed) {
            if !list_open {
                out.push_str(r#"<ul class="text-list">"#);
                list_open = true;
            }
            out.push_str(&format!("<li>{}</li>", format_inline(item)));
        } else {
            if list_open {
                out.push_str("</ul>");
                list_open = false;
            }
            out.push_str(&format!("<p>{}</p>", format_inline(trimmed)));
        }
    }

    if list_open {
        out.push_str("</ul>");
    }

    out
}

// Why: a `*` bullet needs the trailing space, which is what keeps `**bold**` at
// the start of a line from being mistaken for one.
fn bullet_body(line: &str) -> Option<&str> {
    line.strip_prefix("- ").or_else(|| line.strip_prefix("* "))
}

fn format_inline(text: &str) -> String {
    let bolded = wrap_delimited(text, "**", "strong");
    wrap_delimited(&bolded, "`", "code")
}

// Why: this operates on already-escaped text, so it can only introduce the tag
// it is asked for. An odd number of delimiters means the author wrote a literal
// marker rather than a pair, and the text is returned untouched.
fn wrap_delimited(text: &str, delimiter: &str, tag: &str) -> String {
    let parts: Vec<&str> = text.split(delimiter).collect();
    if parts.len() < 3 || parts.len().is_multiple_of(2) {
        return text.to_owned();
    }

    let mut out = String::with_capacity(text.len());
    for (index, part) in parts.iter().enumerate() {
        if index % 2 == 0 {
            out.push_str(part);
        } else {
            out.push_str(&format!("<{tag}>{part}</{tag}>"));
        }
    }
    out
}

const fn text_styles() -> &'static str {
    include_str!("assets/css/text.css")
}

fn payload_language(artifact: &Artifact) -> Option<String> {
    artifact.parts.iter().find_map(|part| {
        part.as_data()?
            .get("language")?
            .as_str()
            .map(ToOwned::to_owned)
    })
}
