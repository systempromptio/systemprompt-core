//! Unit tests for `McpOutputSchema` trait impls.

use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::artifacts::{
    AudioArtifact, ChartArtifact, CliArtifact, CopyPasteTextArtifact, DashboardArtifact,
    ImageArtifact, ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
    VideoArtifact,
};

#[test]
fn text_artifact_artifact_type_str() {
    assert_eq!(
        TextArtifact::artifact_type(),
        TextArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn copy_paste_text_artifact_type() {
    assert_eq!(
        CopyPasteTextArtifact::artifact_type(),
        CopyPasteTextArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn audio_artifact_type() {
    assert_eq!(
        AudioArtifact::artifact_type(),
        AudioArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn dashboard_artifact_type() {
    assert_eq!(
        DashboardArtifact::artifact_type(),
        DashboardArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn presentation_card_artifact_type() {
    assert_eq!(
        PresentationCardArtifact::artifact_type(),
        PresentationCardArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn table_artifact_type() {
    assert_eq!(
        TableArtifact::artifact_type(),
        TableArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn list_artifact_type() {
    assert_eq!(
        ListArtifact::artifact_type(),
        ListArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn chart_artifact_type() {
    assert_eq!(
        ChartArtifact::artifact_type(),
        ChartArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn image_artifact_type() {
    assert_eq!(
        ImageArtifact::artifact_type(),
        ImageArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn video_artifact_type() {
    assert_eq!(
        VideoArtifact::artifact_type(),
        VideoArtifact::ARTIFACT_TYPE_STR
    );
}

#[test]
fn validated_schema_includes_x_artifact_type() {
    let schema = <TextArtifact as McpOutputSchema>::validated_schema();
    let value = schema
        .get("x-artifact-type")
        .and_then(|v| v.as_str())
        .expect("x-artifact-type tag");
    assert_eq!(value, TextArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn validated_schema_for_each_artifact() {
    // Smoke: every artifact's validated_schema() should produce a Value
    // (non-null JSON) whose top-level is an object containing our tag.
    let schemas = [
        <TextArtifact as McpOutputSchema>::validated_schema(),
        <CopyPasteTextArtifact as McpOutputSchema>::validated_schema(),
        <AudioArtifact as McpOutputSchema>::validated_schema(),
        <DashboardArtifact as McpOutputSchema>::validated_schema(),
        <PresentationCardArtifact as McpOutputSchema>::validated_schema(),
        <TableArtifact as McpOutputSchema>::validated_schema(),
        <ListArtifact as McpOutputSchema>::validated_schema(),
        <ChartArtifact as McpOutputSchema>::validated_schema(),
        <ImageArtifact as McpOutputSchema>::validated_schema(),
        <VideoArtifact as McpOutputSchema>::validated_schema(),
    ];
    for schema in &schemas {
        assert!(schema.is_object(), "schema should be a JSON object");
        assert!(schema.get("x-artifact-type").is_some());
    }
}

/// The MCP spec requires a tool's `outputSchema` root to carry
/// `"type": "object"`, and Claude Desktop parks the entire server on the
/// first tool that violates it. `CliArtifact` is the case that regressed:
/// a tagged enum whose schemars output is a bare `oneOf` with no `type`.
#[test]
fn every_validated_schema_is_an_object_schema() {
    let schemas = [
        <CliArtifact as McpOutputSchema>::validated_schema(),
        <TextArtifact as McpOutputSchema>::validated_schema(),
        <CopyPasteTextArtifact as McpOutputSchema>::validated_schema(),
        <AudioArtifact as McpOutputSchema>::validated_schema(),
        <DashboardArtifact as McpOutputSchema>::validated_schema(),
        <PresentationCardArtifact as McpOutputSchema>::validated_schema(),
        <TableArtifact as McpOutputSchema>::validated_schema(),
        <ListArtifact as McpOutputSchema>::validated_schema(),
        <ChartArtifact as McpOutputSchema>::validated_schema(),
        <ImageArtifact as McpOutputSchema>::validated_schema(),
        <VideoArtifact as McpOutputSchema>::validated_schema(),
    ];
    for schema in &schemas {
        assert_eq!(
            schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "outputSchema root must be an object schema: {}",
            schema
                .get("x-artifact-type")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        );
    }
}

/// The drift gate: the outputSchema a tool advertises and the
/// structuredContent it serializes come from the SAME type via two different
/// derives (schemars vs serde). A `skip_serializing_if` the schema still
/// marks required — or any other disagreement between the two views — makes
/// every client-side validator reject the payload. Serialize minimal
/// (None/empty) instances of every artifact and validate each against its own
/// advertised schema.
#[test]
fn minimal_payloads_of_every_artifact_validate_against_their_advertised_schema() {
    use systemprompt_models::artifacts::{Column, NoticeLine};

    fn assert_valid<T: McpOutputSchema + serde::Serialize>(value: &T, label: &str) {
        let schema = <T as McpOutputSchema>::validated_schema();
        let validator = jsonschema::validator_for(&schema)
            .unwrap_or_else(|e| panic!("{label}: schema does not compile: {e}"));
        let payload = serde_json::to_value(value).expect(label);
        if let Err(e) = validator.validate(&payload) {
            panic!("{label}: payload does not match its advertised schema: {e}\n{payload}");
        }
    }

    assert_valid(&TextArtifact::new(""), "text");
    assert_valid(&CopyPasteTextArtifact::new(""), "copy_paste_text");
    assert_valid(&AudioArtifact::new("https://example.com/a.mp3"), "audio");
    assert_valid(&DashboardArtifact::new(""), "dashboard");
    assert_valid(&PresentationCardArtifact::new(""), "presentation_card");
    assert_valid(
        &TableArtifact::new(vec![Column::new(
            "c",
            systemprompt_models::artifacts::ColumnType::String,
        )]),
        "table",
    );
    assert_valid(&ListArtifact::new(), "list");
    assert_valid(
        &ChartArtifact::new("", systemprompt_models::artifacts::ChartType::Bar),
        "chart",
    );
    assert_valid(&ImageArtifact::new("https://example.com/i.png"), "image");
    assert_valid(&VideoArtifact::new("https://example.com/v.mp4"), "video");

    let cli_variants: Vec<(CliArtifact, &str)> = vec![
        (CliArtifact::text(TextArtifact::new("")), "cli:text"),
        (
            CliArtifact::table(TableArtifact::new(vec![Column::new(
                "c",
                systemprompt_models::artifacts::ColumnType::String,
            )])),
            "cli:table",
        ),
        (CliArtifact::list(ListArtifact::new()), "cli:list"),
        (
            CliArtifact::copy_paste_text(CopyPasteTextArtifact::new("")),
            "cli:copy_paste_text",
        ),
        (
            CliArtifact::dashboard(DashboardArtifact::new("")),
            "cli:dashboard",
        ),
        (
            CliArtifact::chart(ChartArtifact::new(
                "",
                systemprompt_models::artifacts::ChartType::Bar,
            )),
            "cli:chart",
        ),
        (
            CliArtifact::audio(AudioArtifact::new("https://example.com/a.mp3")),
            "cli:audio",
        ),
        (
            CliArtifact::image(ImageArtifact::new("https://example.com/i.png")),
            "cli:image",
        ),
        (
            CliArtifact::video(VideoArtifact::new("https://example.com/v.mp4")),
            "cli:video",
        ),
        (
            CliArtifact::presentation_card(PresentationCardArtifact::new("")),
            "cli:card",
        ),
        (
            CliArtifact::message(systemprompt_models::artifacts::MessageArtifact::new(vec![
                NoticeLine::new("info", "m"),
            ])),
            "cli:message",
        ),
    ];
    let schema = <CliArtifact as McpOutputSchema>::validated_schema();
    let validator = jsonschema::validator_for(&schema).expect("CliArtifact schema compiles");
    for (artifact, label) in &cli_variants {
        let payload = serde_json::to_value(artifact).expect(label);
        if let Err(e) = validator.validate(&payload) {
            panic!(
                "{label}: payload does not match the advertised CliArtifact schema: {e}\n{payload}"
            );
        }
    }
}

#[test]
fn artifact_type_name_default_returns_static_str() {
    let text = TextArtifact::new("hello");
    assert_eq!(text.artifact_type_name(), TextArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn artifact_title_text_artifact_optional_some() {
    let text = TextArtifact::new("hello").with_title("hi");
    assert_eq!(text.artifact_title(), Some("hi".to_string()));
}

#[test]
fn artifact_title_text_artifact_none() {
    let text = TextArtifact::new("hello");
    assert_eq!(text.artifact_title(), None);
}
