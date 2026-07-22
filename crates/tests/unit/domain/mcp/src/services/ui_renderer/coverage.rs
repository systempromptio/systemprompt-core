use systemprompt_mcp::services::ui_renderer::registry::create_default_registry;
use systemprompt_models::artifacts::{
    AudioArtifact, ChartArtifact, CopyPasteTextArtifact, DashboardArtifact, ImageArtifact,
    ListArtifact, MessageArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
    VideoArtifact,
};
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, DataPart, Part};

// A variant added to the union without a renderer silently degrades to a raw
// JSON dump in the host, so the registry must cover this list exactly.
const CLI_ARTIFACT_TYPES: &[&str] = &[
    TableArtifact::ARTIFACT_TYPE_STR,
    ListArtifact::ARTIFACT_TYPE_STR,
    TextArtifact::ARTIFACT_TYPE_STR,
    CopyPasteTextArtifact::ARTIFACT_TYPE_STR,
    DashboardArtifact::ARTIFACT_TYPE_STR,
    ChartArtifact::ARTIFACT_TYPE_STR,
    AudioArtifact::ARTIFACT_TYPE_STR,
    ImageArtifact::ARTIFACT_TYPE_STR,
    VideoArtifact::ARTIFACT_TYPE_STR,
    PresentationCardArtifact::ARTIFACT_TYPE_STR,
    MessageArtifact::ARTIFACT_TYPE_STR,
];

fn enveloped(payload: serde_json::Value) -> Artifact {
    let data = match payload {
        serde_json::Value::Object(map) => map,
        other => panic!("artifact payload must be an object, got {other}"),
    };

    Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: None,
        description: None,
        parts: vec![Part::Data(DataPart { data })],
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            "cli".to_string(),
            systemprompt_identifiers::ContextId::generate(),
            systemprompt_identifiers::TaskId::generate(),
        ),
    }
}

#[test]
fn default_registry_covers_every_cli_artifact_variant() {
    let registry = create_default_registry();
    let missing: Vec<&&str> = CLI_ARTIFACT_TYPES
        .iter()
        .filter(|t| !registry.supports(t))
        .collect();

    assert!(
        missing.is_empty(),
        "CliArtifact variants without a UI renderer: {missing:?}"
    );
}

#[tokio::test]
async fn presentation_card_renders_sections_not_json() {
    let registry = create_default_registry();
    let artifact = enveloped(serde_json::json!({
        "artifact_type": "presentation_card",
        "title": "Platform Overview",
        "sections": [{"heading": "Total users", "content": "15"}],
        "theme": "gradient"
    }));

    let resource = registry.render(&artifact).await.expect("card renders");

    assert!(resource.html.contains("Platform Overview"));
    assert!(resource.html.contains("Total users"));
    assert!(resource.html.contains("card-section"));
}

#[tokio::test]
async fn presentation_card_renders_ctas_and_subtitle() {
    let registry = create_default_registry();
    let artifact = enveloped(serde_json::json!({
        "artifact_type": "presentation_card",
        "title": "Release Report",
        "subtitle": "v1.2.3 rollout",
        "sections": [{"heading": "Status", "content": "line one\nline two", "icon": "S"}],
        "ctas": [
            {"id": "approve", "label": "Approve", "message": "approve it", "variant": "primary", "icon": ">"},
            {"id": "reject", "label": "Reject", "message": "reject it", "variant": "secondary"}
        ]
    }));

    let resource = registry.render(&artifact).await.expect("card renders");

    assert!(resource.html.contains("v1.2.3 rollout"));
    assert!(resource.html.contains("card-section-icon"));
    assert!(resource.html.contains(r#"data-cta-id="approve""#));
    assert!(resource.html.contains("card-cta-secondary"));
    assert!(resource.html.contains("card-ctas"));
    assert!(resource.html.contains("window.CARD_CTAS"));
}

#[tokio::test]
async fn table_renders_items_rows() {
    let registry = create_default_registry();
    let artifact = enveloped(serde_json::json!({
        "artifact_type": "table",
        "columns": [{"name": "email", "column_type": "text"}],
        "items": [{"email": "ed@example.com"}]
    }));

    let resource = registry.render(&artifact).await.expect("table renders");

    assert!(resource.html.contains("data-table"));
    assert!(resource.html.contains("ed@example.com"));
}

#[tokio::test]
async fn message_renders_severity_lines() {
    let registry = create_default_registry();
    let artifact = enveloped(serde_json::json!({
        "artifact_type": "message",
        "messages": [{"level": "warning", "text": "Nothing to do"}]
    }));

    let resource = registry.render(&artifact).await.expect("message renders");

    assert!(resource.html.contains("notice-warning"));
    assert!(resource.html.contains("Nothing to do"));
}

#[tokio::test]
async fn copy_paste_text_renders_preformatted() {
    let registry = create_default_registry();
    let artifact = enveloped(serde_json::json!({
        "artifact_type": "copy_paste_text",
        "title": "Command",
        "content": "systemprompt admin users list"
    }));

    let resource = registry
        .render(&artifact)
        .await
        .expect("copy paste text renders");

    assert!(resource.html.contains("<pre><code>"));
    assert!(resource.html.contains("systemprompt admin users list"));
}

#[tokio::test]
async fn every_rendered_document_negotiates_its_height() {
    let registry = create_default_registry();
    let artifact = enveloped(serde_json::json!({
        "artifact_type": "text",
        "content": "hello"
    }));

    let resource = registry.render(&artifact).await.expect("text renders");

    assert!(resource.html.contains("ui/notifications/size-changed"));
}
