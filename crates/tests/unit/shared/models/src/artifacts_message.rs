//! Tests for the message/notice artifact: construction, serde envelope, and
//! the schema it advertises.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::json;
use systemprompt_models::artifacts::message::{MessageArtifact, NoticeLine};
use systemprompt_models::artifacts::traits::Artifact;
use systemprompt_models::artifacts::types::ArtifactType;

#[test]
fn notice_line_new_accepts_borrowed_and_owned() {
    let from_str = NoticeLine::new("info", "hello");
    let from_owned = NoticeLine::new(String::from("info"), String::from("hello"));

    assert_eq!(from_str.level, "info");
    assert_eq!(from_str.text, "hello");
    assert_eq!(from_owned.level, from_str.level);
    assert_eq!(from_owned.text, from_str.text);
}

#[test]
fn new_stamps_the_artifact_type_discriminant() {
    let artifact = MessageArtifact::new(vec![NoticeLine::new("warning", "disk almost full")]);

    assert_eq!(artifact.artifact_type, MessageArtifact::ARTIFACT_TYPE_STR);
    assert_eq!(artifact.artifact_type, "message");
    assert_eq!(artifact.messages.len(), 1);
}

#[test]
fn empty_notice_list_is_representable() {
    let artifact = MessageArtifact::new(vec![]);
    assert!(artifact.messages.is_empty());
    assert_eq!(artifact.artifact_type(), ArtifactType::Message);
}

#[test]
fn artifact_type_trait_reports_message() {
    let artifact = MessageArtifact::new(vec![NoticeLine::new("error", "boom")]);
    assert_eq!(artifact.artifact_type(), ArtifactType::Message);
}

#[test]
fn serializes_with_hyphenated_artifact_type_key() {
    let artifact = MessageArtifact::new(vec![NoticeLine::new("success", "done")]);
    let value = serde_json::to_value(&artifact).unwrap();

    assert_eq!(value["x-artifact-type"], json!("message"));
    assert_eq!(value["messages"][0]["level"], json!("success"));
    assert_eq!(value["messages"][0]["text"], json!("done"));
    assert!(
        value.as_object().unwrap().get("artifact_type").is_none(),
        "the rename must not leak the Rust field name"
    );
}

#[test]
fn deserializing_without_type_key_falls_back_to_default() {
    let artifact: MessageArtifact = serde_json::from_value(json!({
        "messages": [{ "level": "info", "text": "implicit" }]
    }))
    .unwrap();

    assert_eq!(
        artifact.artifact_type,
        MessageArtifact::ARTIFACT_TYPE_STR,
        "a legacy envelope without x-artifact-type still deserializes"
    );
    assert_eq!(artifact.messages[0].text, "implicit");
}

#[test]
fn round_trips_through_json() {
    let original = MessageArtifact::new(vec![
        NoticeLine::new("info", "first"),
        NoticeLine::new("error", "second"),
    ]);
    let restored: MessageArtifact =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();

    assert_eq!(restored.messages.len(), 2);
    assert_eq!(restored.messages[0].level, "info");
    assert_eq!(restored.messages[1].text, "second");
    assert_eq!(restored.artifact_type, original.artifact_type);
}

#[test]
fn missing_messages_field_is_rejected() {
    let err = serde_json::from_value::<MessageArtifact>(json!({
        "x-artifact-type": "message"
    }))
    .unwrap_err();
    assert!(
        err.to_string().contains("messages"),
        "unexpected error: {err}"
    );
}

#[test]
fn schema_declares_required_notice_fields() {
    let schema = MessageArtifact::new(vec![]).to_schema();

    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["x-artifact-type"], json!("message"));
    assert_eq!(schema["required"], json!(["messages"]));

    let items = &schema["properties"]["messages"]["items"];
    assert_eq!(schema["properties"]["messages"]["type"], json!("array"));
    assert_eq!(items["required"], json!(["level", "text"]));
    assert_eq!(items["properties"]["level"]["type"], json!("string"));
    assert_eq!(items["properties"]["text"]["type"], json!("string"));
}
