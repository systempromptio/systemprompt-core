//! Audio and video artifact renderers.
//!
//! [`AudioRenderer`] and [`VideoRenderer`] render their model types into HTML
//! [`UiResource`]s built around a native media element, widening the CSP
//! `media-src` to permit `https:`, `data:`, and `blob:` sources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script};
use super::typed::artifact_payload;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspBuilder, CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::{ArtifactType, AudioArtifact, VideoArtifact};

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioRenderer;

impl AudioRenderer {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UiRenderer for AudioRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Audio
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let audio: AudioArtifact = artifact_payload(artifact)?;
        let title = audio
            .title
            .as_deref()
            .or(artifact.title.as_deref())
            .unwrap_or("Audio");

        let body = format!(
            r#"<div class="container">
    <h1 class="mcp-app-title">{title}</h1>
    {artist_html}
    <div class="media-frame">
        {artwork_html}
        <audio class="media-element" src="{src}"{type_attr}{flags}>Your browser cannot play this audio. <a href="{src}">Download the file</a> instead.</audio>
    </div>
</div>"#,
            title = html_escape(title),
            artist_html = audio.artist.as_ref().map_or_else(String::new, |a| format!(
                r#"<p class="mcp-app-description">{}</p>"#,
                html_escape(a)
            )),
            artwork_html = audio.artwork.as_ref().map_or_else(String::new, |a| format!(
                r#"<img class="media-artwork" src="{}" alt="">"#,
                html_escape(a)
            )),
            src = html_escape(&audio.src),
            type_attr = mime_attr(audio.mime_type.as_deref()),
            flags = Playback {
                controls: audio.controls,
                autoplay: audio.autoplay,
                repeat: audio.loop_playback,
                muted: false,
            }
            .to_attributes(),
        );

        Ok(media_resource(title, &body, self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        media_csp()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VideoRenderer;

impl VideoRenderer {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UiRenderer for VideoRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Video
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let video: VideoArtifact = artifact_payload(artifact)?;
        let title = artifact.title.as_deref().unwrap_or("Video");

        let body = format!(
            r#"<div class="container">
    <h1 class="mcp-app-title">{title}</h1>
    <div class="media-frame">
        <video class="media-element" src="{src}"{type_attr}{poster}{flags}>Your browser cannot play this video. <a href="{src}">Download the file</a> instead.</video>
    </div>
    {caption_html}
</div>"#,
            title = html_escape(title),
            src = html_escape(&video.src),
            type_attr = mime_attr(video.mime_type.as_deref()),
            poster = video
                .poster
                .as_ref()
                .map_or_else(String::new, |p| format!(r#" poster="{}""#, html_escape(p))),
            flags = Playback {
                controls: video.controls,
                autoplay: video.autoplay,
                repeat: video.loop_playback,
                muted: video.muted,
            }
            .to_attributes(),
            caption_html = video.caption.as_ref().map_or_else(String::new, |c| format!(
                r#"<p class="media-caption">{}</p>"#,
                html_escape(c)
            )),
        );

        Ok(media_resource(title, &body, self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        media_csp()
    }
}

fn media_resource(title: &str, body: &str, csp: CspPolicy) -> UiResource {
    let html = HtmlBuilder::new(title)
        .add_style(base_styles())
        .add_style(media_styles())
        .body(body)
        .add_script(&format!(
            "{bridge}\n{app}",
            bridge = mcp_app_bridge_script(),
            app = include_str!("assets/js/media.js"),
        ))
        .build();

    UiResource::new(html).with_csp(csp)
}

fn media_csp() -> CspPolicy {
    CspBuilder::strict()
        .media_src(vec![
            "'self'".to_owned(),
            "https:".to_owned(),
            "data:".to_owned(),
            "blob:".to_owned(),
        ])
        .build()
}

fn mime_attr(mime_type: Option<&str>) -> String {
    mime_type.map_or_else(String::new, |m| format!(r#" type="{}""#, html_escape(m)))
}

#[derive(Debug, Clone, Copy, Default)]
struct Playback {
    controls: bool,
    autoplay: bool,
    repeat: bool,
    muted: bool,
}

impl Playback {
    fn to_attributes(self) -> String {
        let mut flags = String::new();
        // Why: `controls: false` with no autoplay renders an element with no
        // affordance and no script to drive it — indistinguishable from a
        // broken artifact. Controls are forced back on unless the media plays
        // by itself.
        if self.controls || !self.autoplay {
            flags.push_str(" controls");
        }
        if self.autoplay {
            flags.push_str(" autoplay");
        }
        if self.repeat {
            flags.push_str(" loop");
        }
        if self.muted {
            flags.push_str(" muted");
        }
        flags
    }
}

const fn media_styles() -> &'static str {
    include_str!("assets/css/media.css")
}
