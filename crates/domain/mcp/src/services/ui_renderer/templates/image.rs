use super::html::{base_styles, html_escape, mcp_app_bridge_script, HtmlBuilder};
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
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
            if let Some(file) = part.as_file() {
                if let Some(bytes) = file.get("bytes").and_then(|v| v.as_str()) {
                    let mime = file
                        .get("mimeType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("image/png");
                    data.src = format!("data:{};base64,{}", mime, bytes);
                } else if let Some(uri) = file.get("uri").and_then(|v| v.as_str()) {
                    data.src = uri.to_string();
                }
            }

            if let Some(part_data) = part.as_data() {
                if let Some(src) = part_data
                    .get("src")
                    .or_else(|| part_data.get("url"))
                    .and_then(|v| v.as_str())
                {
                    data.src = src.to_string();
                }
                if let Some(alt) = part_data.get("alt").and_then(|v| v.as_str()) {
                    data.alt = Some(alt.to_string());
                }
                if let Some(caption) = part_data.get("caption").and_then(|v| v.as_str()) {
                    data.caption = Some(caption.to_string());
                }
                if let Some(width) = part_data.get("width").and_then(JsonValue::as_u64) {
                    data.width = Some(width as u32);
                }
                if let Some(height) = part_data.get("height").and_then(JsonValue::as_u64) {
                    data.height = Some(height as u32);
                }
            }
        }

        if let Some(hints) = &artifact.metadata.rendering_hints {
            if let Some(alt) = hints.get("alt").and_then(|v| v.as_str()) {
                data.alt = Some(alt.to_string());
            }
            if let Some(caption) = hints.get("caption").and_then(|v| v.as_str()) {
                data.caption = Some(caption.to_string());
            }
        }

        data
    }
}

#[derive(Default)]
struct ImageData {
    src: String,
    alt: Option<String>,
    caption: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[async_trait]
impl UiRenderer for ImageRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Image
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let image_data = Self::extract_image_data(artifact);
        let title = artifact.name.as_deref().unwrap_or("Image");

        let alt_text = image_data.alt.as_deref().unwrap_or(title);

        let size_attrs = match (image_data.width, image_data.height) {
            (Some(w), Some(h)) => format!(r#" width="{}" height="{}""#, w, h),
            (Some(w), None) => format!(r#" width="{}""#, w),
            (None, Some(h)) => format!(r#" height="{}""#, h),
            (None, None) => String::new(),
        };

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    <figure class="image-figure">
        <div class="image-wrapper">
            <img src="{src}" alt="{alt}" class="artifact-image"{size_attrs} loading="lazy">
            <div class="image-controls">
                <button class="control-btn zoom-in" title="Zoom in">+</button>
                <button class="control-btn zoom-out" title="Zoom out">−</button>
                <button class="control-btn zoom-reset" title="Reset zoom">⟲</button>
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
                .map(|d| format!(r#"<p class="mcp-app-description">{}</p>"#, html_escape(d)))
                .unwrap_or_default(),
            src = html_escape(&image_data.src),
            alt = html_escape(alt_text),
            size_attrs = size_attrs,
            caption_html = image_data
                .caption
                .as_ref()
                .map(|c| format!(
                    r#"<figcaption class="image-caption">{}</figcaption>"#,
                    html_escape(c)
                ))
                .unwrap_or_default(),
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
        policy.img_src.push("https:".to_string());
        policy.img_src.push("blob:".to_string());
        policy
    }
}

const fn image_styles() -> &'static str {
    include_str!("assets/css/image.css")
}
