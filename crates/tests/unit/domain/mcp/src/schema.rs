//! Unit tests for `McpOutputSchema` trait impls.

use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::artifacts::{
    AudioArtifact, ChartArtifact, CopyPasteTextArtifact, DashboardArtifact, ImageArtifact,
    ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact, VideoArtifact,
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

#[test]
fn artifact_type_name_default_returns_static_str() {
    let ctx = test_request_context();
    let text = TextArtifact::new("hello", &ctx);
    assert_eq!(text.artifact_type_name(), TextArtifact::ARTIFACT_TYPE_STR);
}

#[test]
fn artifact_title_text_artifact_optional_some() {
    let ctx = test_request_context();
    let text = TextArtifact::new("hello", &ctx).with_title("hi");
    assert_eq!(text.artifact_title(), Some("hi".to_string()));
}

#[test]
fn artifact_title_text_artifact_none() {
    let ctx = test_request_context();
    let text = TextArtifact::new("hello", &ctx);
    assert_eq!(text.artifact_title(), None);
}

fn test_request_context() -> systemprompt_models::RequestContext {
    use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
    systemprompt_models::RequestContext::new(
        SessionId::new("s"),
        TraceId::new("t"),
        ContextId::new("00000000-0000-4000-8000-000000000001"),
        AgentName::new("a"),
    )
}
