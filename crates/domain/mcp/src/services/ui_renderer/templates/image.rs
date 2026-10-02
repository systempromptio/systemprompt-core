//! Image artifact renderer.
//!
//! [`ImageRenderer`] renders an image [`Artifact`] — sourced from inline
//! base64 file parts, a remote URI, or data-part metadata — into an HTML
//! [`UiResource`] with caption and zoom controls, widening the CSP `img-src`
//! to permit `https:` and `blob:` sources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{
    HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script, safe_url, unsafe_url_error,
};
use super::typed::lenient;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::{Artifact, Part};
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, Copy, Default)]
pub struct ImageRenderer;

impl ImageRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_image_data(artifact: &Artifact) -> ImageData {
        let mut data = ImageData::default();

        for part in &artifact.parts {
            match part {
                Part::File(file_part) => {
                    let file = &file_part.file;
                    if let Some(bytes) = &file.bytes {
                        let mime = file.mime_type.as_deref().unwrap_or("image/png");
                        data.src = format!("data:{mime};base64,{bytes}");
                    } else if let Some(url) = &file.url {
                        url.clone_into(&mut data.src);
                    }
                },
                Part::Data(data_part) => {
                    let Ok(fields) =
                        ImageDataFields::deserialize(JsonValue::Object(data_part.data.clone()))
                    else {
                        continue;
                    };
                    if let Some(src) = fields.src.or(fields.url) {
                        data.src = src;
                    }
                    data.alt = fields.alt.or(data.alt);
                    data.caption = fields.caption.or(data.caption);
                    data.width = fields.width.or(data.width);
                    data.height = fields.height.or(data.height);
                },
                Part::Text(_) => {},
            }
        }

        if let Some(hints) = artifact
            .metadata
            .rendering_hints
            .clone()
            .and_then(|hints| ImageHints::deserialize(hints).ok())
        {
            data.alt = hints.alt.or(data.alt);
            data.caption = hints.caption.or(data.caption);
        }

        data
    }

    fn render_empty(&self, title: &str) -> UiResource {
        let body = format!(
            r#"<div class="container">
    {title_html}
    <p class="image-empty">No image to show.</p>
</div>"#,
            title_html = if title.is_empty() {
                String::new()
            } else {
                format!(r#"<h1 class="mcp-app-title">{}</h1>"#, html_escape(title))
            },
        );
        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(image_styles())
            .body(&body)
            .add_script(mcp_app_bridge_script())
            .build();
        UiResource::new(html).with_csp(self.csp_policy())
    }
}

#[derive(Deserialize)]
struct ImageDataFields {
    #[serde(default, deserialize_with = "lenient")]
    src: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    url: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    alt: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    caption: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    width: Option<u32>,
    #[serde(default, deserialize_with = "lenient")]
    height: Option<u32>,
}

#[derive(Deserialize)]
struct ImageHints {
    #[serde(default, deserialize_with = "lenient")]
    alt: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    caption: Option<String>,
}

#[derive(Default)]
struct ImageData {
    src: String,
    alt: Option<String>,
    caption: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

impl UiRenderer for ImageRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Image
    }

    fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let image_data = Self::extract_image_data(artifact);
        let title = artifact.title.as_deref().unwrap_or("Image");

        let alt_text = image_data.alt.as_deref().unwrap_or(title);

        let size_attrs = match (image_data.width, image_data.height) {
            (Some(w), Some(h)) => format!(r#" width="{}" height="{}""#, w, h),
            (Some(w), None) => format!(r#" width="{}""#, w),
            (None, Some(h)) => format!(r#" height="{}""#, h),
            (None, None) => String::new(),
        };

        if image_data.src.is_empty() {
            return Ok(self.render_empty(title));
        }
        let src = safe_url(&image_data.src)
            .ok_or_else(|| unsafe_url_error("image src", &image_data.src))?;

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    <figure class="image-figure">
        <div class="image-wrapper is-loading skeleton">
            <img src="{src}" alt="{alt}" class="artifact-image"{size_attrs} decoding="async">
            <div class="image-controls">
                <button class="control-btn zoom-in" type="button" aria-label="Zoom in"><span aria-hidden="true">+</span></button>
                <button class="control-btn zoom-out" type="button" aria-label="Zoom out"><span aria-hidden="true">−</span></button>
                <button class="control-btn zoom-reset" type="button" aria-label="Reset zoom"><span aria-hidden="true">⟲</span></button>
                <span class="zoom-status" id="zoom-status" role="status" aria-live="polite"></span>
            </div>
        </div>
        {caption_html}
    </figure>
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
            src = src,
            alt = html_escape(alt_text),
            size_attrs = size_attrs,
            caption_html = image_data
                .caption
                .as_ref()
                .map_or_else(String::new, |c| format!(
                    r#"<figcaption class="image-caption">{}</figcaption>"#,
                    html_escape(c)
                )),
        );

        let script = format!(
            "{bridge}\n{app}",
            bridge = mcp_app_bridge_script(),
            app = include_str!("assets/js/image.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(image_styles())
            .body(&body)
            .add_script(&script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        let mut policy = CspPolicy::strict();
        policy.img_src.push("https:".to_owned());
        policy.img_src.push("blob:".to_owned());
        policy
    }
}

const fn image_styles() -> &'static str {
    include_str!("assets/css/image.css")
}
