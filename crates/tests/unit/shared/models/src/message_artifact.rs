//! Unit tests for the message/notice artifact and its CliArtifact variant.

use systemprompt_models::artifacts::{
    Artifact, ArtifactType, CliArtifact, MessageArtifact, NoticeLine,
};

#[test]
fn message_artifact_serializes_with_type_tag() {
    let artifact = MessageArtifact::new(vec![
        NoticeLine::new("warning", "no rows"),
        NoticeLine::new("info", "try again"),
    ]);

    let json = serde_json::to_value(&artifact).unwrap();

    assert_eq!(json["x-artifact-type"], "message");
    assert_eq!(json["messages"][0]["level"], "warning");
    assert_eq!(json["messages"][0]["text"], "no rows");
    assert_eq!(json["messages"][1]["level"], "info");
}

#[test]
fn message_artifact_round_trips() {
    let artifact = MessageArtifact::new(vec![NoticeLine::new("error", "boom")]);

    let json = serde_json::to_string(&artifact).unwrap();
    let back: MessageArtifact = serde_json::from_str(&json).unwrap();

    assert_eq!(back.messages.len(), 1);
    assert_eq!(back.messages[0].level, "error");
    assert_eq!(back.messages[0].text, "boom");
}

#[test]
fn cli_artifact_message_variant_metadata() {
    let artifact = CliArtifact::message(MessageArtifact::new(vec![NoticeLine::new("info", "hi")]));

    assert!(matches!(artifact, CliArtifact::Message { .. }));
    assert_eq!(artifact.artifact_type_str(), "message");
    assert!(artifact.title().is_none());
}

#[test]
fn cli_artifact_message_serializes_with_enum_tag() {
    let artifact = CliArtifact::message(MessageArtifact::new(vec![NoticeLine::new("info", "hi")]));

    let json = serde_json::to_value(&artifact).unwrap();

    assert_eq!(json["artifact_type"], "message");
    assert_eq!(json["messages"][0]["text"], "hi");
}

#[test]
fn cli_artifact_message_deserializes_from_wire() {
    let wire = r#"{"artifact_type":"message","messages":[{"level":"warning","text":"x"}]}"#;

    let artifact: CliArtifact = serde_json::from_str(wire).unwrap();

    assert!(matches!(artifact, CliArtifact::Message { .. }));
    assert_eq!(artifact.artifact_type_str(), "message");
}

#[test]
fn message_artifact_type_is_first_class_variant() {
    let artifact = MessageArtifact::new(vec![NoticeLine::new("info", "hi")]);

    assert_eq!(artifact.artifact_type(), ArtifactType::Message);
}

#[test]
fn artifact_type_message_serde_matches_wire_tag() {
    let json = serde_json::to_string(&ArtifactType::Message).unwrap();
    assert_eq!(json, "\"message\"");

    let back: ArtifactType = serde_json::from_str("\"message\"").unwrap();
    assert_eq!(back, ArtifactType::Message);
}
