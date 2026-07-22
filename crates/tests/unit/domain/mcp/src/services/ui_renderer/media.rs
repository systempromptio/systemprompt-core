use systemprompt_identifiers::{ArtifactId, ContextId, TaskId};
use systemprompt_mcp::services::ui_renderer::UiRenderer;
use systemprompt_mcp::services::ui_renderer::templates::{AudioRenderer, VideoRenderer};
use systemprompt_models::artifacts::ArtifactType;
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, DataPart, Part};

fn media_artifact(kind: &str, title: Option<&str>, payload: serde_json::Value) -> Artifact {
    let serde_json::Value::Object(data) = payload else {
        panic!("payload must be an object");
    };
    Artifact {
        id: ArtifactId::generate(),
        title: title.map(String::from),
        description: None,
        parts: vec![Part::Data(DataPart { data })],
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            kind.to_string(),
            ContextId::generate(),
            TaskId::generate(),
        ),
    }
}

#[test]
fn audio_renderer_declares_audio_artifact_type() {
    assert_eq!(AudioRenderer::new().artifact_type(), ArtifactType::Audio);
    assert_eq!(VideoRenderer::new().artifact_type(), ArtifactType::Video);
}

#[tokio::test]
async fn audio_render_full_payload_includes_artist_artwork_and_flags() {
    let artifact = media_artifact(
        "audio",
        None,
        serde_json::json!({
            "src": "https://cdn.example.com/track.mp3",
            "mime_type": "audio/mpeg",
            "title": "Night Drive",
            "artist": "The <Bots>",
            "artwork": "https://cdn.example.com/cover.png",
            "controls": true,
            "autoplay": true,
            "loop": true
        }),
    );

    let resource = AudioRenderer::new()
        .render(&artifact)
        .await
        .expect("render");

    assert!(resource.html.contains("Night Drive"));
    assert!(resource.html.contains("The &lt;Bots&gt;"));
    assert!(
        resource
            .html
            .contains(r#"<img class="media-artwork" src="https://cdn.example.com/cover.png""#)
    );
    assert!(
        resource
            .html
            .contains(r#"src="https://cdn.example.com/track.mp3""#)
    );
    assert!(resource.html.contains(r#" type="audio/mpeg""#));
    assert!(resource.html.contains(" controls autoplay loop"));
    assert!(!resource.html.contains(" muted"));
}

#[tokio::test]
async fn audio_render_minimal_payload_falls_back_to_artifact_title() {
    let artifact = media_artifact(
        "audio",
        Some("From Artifact"),
        serde_json::json!({"src": "blob:abc"}),
    );

    let resource = AudioRenderer::new()
        .render(&artifact)
        .await
        .expect("render");

    assert!(resource.html.contains("From Artifact"));
    assert!(!resource.html.contains(r#"<p class="mcp-app-description">"#));
    assert!(!resource.html.contains(r#"<img class="media-artwork""#));
    assert!(!resource.html.contains(" type="));
    assert!(resource.html.contains(" controls"));
    assert!(!resource.html.contains("autoplay"));
}

#[tokio::test]
async fn audio_render_without_any_title_uses_default() {
    let artifact = media_artifact("audio", None, serde_json::json!({"src": "a.mp3"}));
    let resource = AudioRenderer::new()
        .render(&artifact)
        .await
        .expect("render");
    assert!(
        resource
            .html
            .contains("<h1 class=\"mcp-app-title\">Audio</h1>")
    );
}

#[tokio::test]
async fn audio_csp_widens_media_src() {
    let policy = AudioRenderer::new().csp_policy();
    let header = policy.to_header_value();
    assert!(header.contains("media-src 'self' https: data: blob:"));
}

#[tokio::test]
async fn video_render_full_payload_includes_poster_caption_and_muted() {
    let artifact = media_artifact(
        "video",
        Some("Launch <Video>"),
        serde_json::json!({
            "src": "https://cdn.example.com/launch.mp4",
            "mime_type": "video/mp4",
            "poster": "https://cdn.example.com/poster.jpg",
            "caption": "Q3 launch & recap",
            "controls": false,
            "autoplay": true,
            "loop": true,
            "muted": true
        }),
    );

    let resource = VideoRenderer::new()
        .render(&artifact)
        .await
        .expect("render");

    assert!(resource.html.contains("Launch &lt;Video&gt;"));
    assert!(
        resource
            .html
            .contains(r#" poster="https://cdn.example.com/poster.jpg""#)
    );
    assert!(resource.html.contains(r#" type="video/mp4""#));
    assert!(resource.html.contains("Q3 launch &amp; recap"));
    assert!(resource.html.contains(" autoplay loop muted"));
    assert!(!resource.html.contains(" controls"));
}

#[tokio::test]
async fn video_render_minimal_payload_uses_default_title_and_omits_extras() {
    let artifact = media_artifact("video", None, serde_json::json!({"src": "v.webm"}));

    let resource = VideoRenderer::new()
        .render(&artifact)
        .await
        .expect("render");

    assert!(
        resource
            .html
            .contains("<h1 class=\"mcp-app-title\">Video</h1>")
    );
    assert!(!resource.html.contains("poster="));
    assert!(!resource.html.contains(r#"<p class="media-caption">"#));
    assert!(resource.html.contains(" controls"));
    assert!(!resource.html.contains("muted"));
}

#[tokio::test]
async fn video_render_rejects_payload_missing_src() {
    let artifact = media_artifact("video", None, serde_json::json!({"caption": "no src"}));
    let result = VideoRenderer::new().render(&artifact).await;
    assert!(result.is_err());
}
