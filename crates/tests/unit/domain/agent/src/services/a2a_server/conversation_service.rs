//! Unit tests for ConversationService pure helpers.
//!
//! Target: crates/domain/agent/src/services/a2a_server/processing/
//! conversation_service.rs

use base64::Engine;
use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Message, MessageRole, Part,
    TextPart,
};
use systemprompt_agent::services::a2a_server::processing::ConversationService;
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::AiContentPart;

fn make_message(parts: Vec<Part>) -> Message {
    Message {
        role: MessageRole::User,
        parts,
        message_id: MessageId::generate(),
        task_id: Some(TaskId::generate()),
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn text(s: &str) -> Part {
    Part::Text(TextPart {
        text: s.to_string(),
    })
}

fn file(name: Option<&str>, mime: Option<&str>, bytes: Option<&str>) -> Part {
    Part::File(FilePart {
        file: FileContent {
            name: name.map(ToString::to_string),
            mime_type: mime.map(ToString::to_string),
            bytes: bytes.map(ToString::to_string),
            url: None,
        },
    })
}

fn data_part(json: serde_json::Value) -> Part {
    let map = match json {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    Part::Data(DataPart { data: map })
}

#[test]
fn extract_message_content_text_only() {
    let msg = make_message(vec![text("hello world")]);
    let (content, parts) = ConversationService::extract_message_content(&msg);
    assert_eq!(content, "hello world");
    assert_eq!(parts.len(), 1);
    matches!(parts[0], AiContentPart::Text { .. });
}

#[test]
fn extract_message_content_first_text_wins_as_content() {
    let msg = make_message(vec![text("first"), text("second")]);
    let (content, parts) = ConversationService::extract_message_content(&msg);
    assert_eq!(content, "first");
    assert_eq!(parts.len(), 2);
}

#[test]
fn extract_message_content_data_part_is_ignored() {
    let msg = make_message(vec![data_part(serde_json::json!({"k": "v"}))]);
    let (content, parts) = ConversationService::extract_message_content(&msg);
    assert!(content.is_empty());
    assert!(parts.is_empty());
}

#[test]
fn extract_message_content_unsupported_file_skipped() {
    let msg = make_message(vec![file(
        Some("a.bin"),
        Some("application/octet-stream"),
        Some("Zm9v"),
    )]);
    let (content, parts) = ConversationService::extract_message_content(&msg);
    assert!(content.is_empty());
    assert!(parts.is_empty());
}

#[test]
fn extract_message_content_file_without_mime_skipped() {
    let msg = make_message(vec![file(Some("a.txt"), None, Some("Zm9v"))]);
    let (_content, parts) = ConversationService::extract_message_content(&msg);
    assert!(parts.is_empty());
}

#[test]
fn extract_message_content_file_without_bytes_skipped() {
    let msg = make_message(vec![file(Some("a.png"), Some("image/png"), None)]);
    let (_content, parts) = ConversationService::extract_message_content(&msg);
    assert!(parts.is_empty());
}

#[test]
fn extract_message_content_mixed_text_and_image() {
    let msg = make_message(vec![
        text("describe this"),
        file(Some("p.png"), Some("image/png"), Some("aW1n")),
    ]);
    let (content, parts) = ConversationService::extract_message_content(&msg);
    assert_eq!(content, "describe this");
    assert_eq!(parts.len(), 2);
    assert!(matches!(parts[1], AiContentPart::Image { .. }));
}

#[test]
fn file_to_content_part_png_image() {
    let part = FilePart {
        file: FileContent {
            name: Some("p.png".to_string()),
            mime_type: Some("image/png".to_string()),
            bytes: Some("AAAA".to_string()),
            url: None,
        },
    };
    let result = ConversationService::file_to_content_part(&part).expect("image");
    match result {
        AiContentPart::Image { mime_type, data } => {
            assert_eq!(mime_type, "image/png");
            assert_eq!(data, "AAAA");
        },
        _ => panic!("expected Image"),
    }
}

#[test]
fn file_to_content_part_audio_wav() {
    let part = FilePart {
        file: FileContent {
            name: Some("a.wav".to_string()),
            mime_type: Some("audio/wav".to_string()),
            bytes: Some("AAAA".to_string()),
            url: None,
        },
    };
    let result = ConversationService::file_to_content_part(&part).expect("audio");
    assert!(matches!(result, AiContentPart::Audio { .. }));
}

#[test]
fn file_to_content_part_video_mp4() {
    let part = FilePart {
        file: FileContent {
            name: None,
            mime_type: Some("video/mp4".to_string()),
            bytes: Some("AAAA".to_string()),
            url: None,
        },
    };
    let result = ConversationService::file_to_content_part(&part).expect("video");
    assert!(matches!(result, AiContentPart::Video { .. }));
}

#[test]
fn file_to_content_part_text_plain_decoded() {
    let encoded = base64::engine::general_purpose::STANDARD.encode("hello text");
    let part = FilePart {
        file: FileContent {
            name: Some("note.txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            bytes: Some(encoded),
            url: None,
        },
    };
    let result = ConversationService::file_to_content_part(&part).expect("text");
    match result {
        AiContentPart::Text { text } => {
            assert!(text.contains("hello text"));
            assert!(text.contains("note.txt"));
            assert!(text.contains("text/plain"));
        },
        _ => panic!("expected Text"),
    }
}

#[test]
fn file_to_content_part_unsupported_mime_returns_none() {
    let part = FilePart {
        file: FileContent {
            name: Some("a.bin".to_string()),
            mime_type: Some("application/octet-stream".to_string()),
            bytes: Some("AAAA".to_string()),
            url: None,
        },
    };
    let result = ConversationService::file_to_content_part(&part);
    assert!(result.is_none());
}

#[test]
fn file_to_content_part_no_mime_returns_none() {
    let part = FilePart {
        file: FileContent {
            name: Some("noext".to_string()),
            mime_type: None,
            bytes: Some("AAAA".to_string()),
            url: None,
        },
    };
    assert!(ConversationService::file_to_content_part(&part).is_none());
}

#[test]
fn file_to_content_part_no_bytes_returns_none() {
    let part = FilePart {
        file: FileContent {
            name: Some("p.png".to_string()),
            mime_type: Some("image/png".to_string()),
            bytes: None,
            url: Some("https://example/p.png".to_string()),
        },
    };
    assert!(ConversationService::file_to_content_part(&part).is_none());
}

#[test]
fn decode_text_file_valid_base64() {
    let encoded = base64::engine::general_purpose::STANDARD.encode("payload");
    let result =
        ConversationService::decode_text_file(&encoded, "x.txt", "text/plain").expect("ok");
    match result {
        AiContentPart::Text { text } => {
            assert!(text.contains("payload"));
            assert!(text.starts_with("[File: x.txt"));
        },
        _ => panic!("expected Text"),
    }
}

#[test]
fn decode_text_file_invalid_base64_returns_none() {
    let result = ConversationService::decode_text_file("!!!!notb64!!", "x.txt", "text/plain");
    assert!(result.is_none());
}

#[test]
fn decode_text_file_invalid_utf8_returns_none() {
    // 0xFF 0xFE is invalid UTF-8.
    let encoded = base64::engine::general_purpose::STANDARD.encode([0xFFu8, 0xFE]);
    let result = ConversationService::decode_text_file(&encoded, "bad", "text/plain");
    assert!(result.is_none());
}

#[test]
fn decode_text_file_empty_payload() {
    let encoded = base64::engine::general_purpose::STANDARD.encode("");
    let result =
        ConversationService::decode_text_file(&encoded, "empty.txt", "text/plain").expect("ok");
    match result {
        AiContentPart::Text { text } => {
            assert!(text.starts_with("[File: empty.txt"));
        },
        _ => panic!("expected Text"),
    }
}

fn make_artifact(parts: Vec<Part>, title: Option<&str>, artifact_type: &str) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: title.map(ToString::to_string),
        description: None,
        parts,
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            artifact_type.to_string(),
            ContextId::generate(),
            TaskId::generate(),
        ),
    }
}

#[test]
fn serialize_artifact_for_context_text_parts() {
    let a = make_artifact(
        vec![text("line one"), text("line two")],
        Some("my-art"),
        "summary",
    );
    let out = ConversationService::serialize_artifact_for_context(&a);
    assert!(out.starts_with("[Artifact: my-art (type: summary)]"));
    assert!(out.contains("line one"));
    assert!(out.contains("line two"));
}

#[test]
fn serialize_artifact_for_context_uses_unnamed_when_no_title() {
    let a = make_artifact(vec![text("payload")], None, "tool_execution");
    let out = ConversationService::serialize_artifact_for_context(&a);
    assert!(out.contains("[Artifact: unnamed (type: tool_execution)]"));
}

#[test]
fn serialize_artifact_for_context_data_part_serialised() {
    let a = make_artifact(
        vec![data_part(serde_json::json!({"k": "v", "n": 42}))],
        Some("data-art"),
        "json",
    );
    let out = ConversationService::serialize_artifact_for_context(&a);
    assert!(out.contains("\"k\""));
    assert!(out.contains("\"v\""));
    assert!(out.contains("42"));
}

#[test]
fn serialize_artifact_for_context_file_part_with_name() {
    let a = make_artifact(
        vec![file(Some("doc.pdf"), Some("application/pdf"), None)],
        Some("file-art"),
        "file",
    );
    let out = ConversationService::serialize_artifact_for_context(&a);
    assert!(out.contains("[File: doc.pdf]"));
}

#[test]
fn serialize_artifact_for_context_file_part_without_name_skipped() {
    let a = make_artifact(
        vec![file(None, Some("application/pdf"), None)],
        Some("file-art"),
        "file",
    );
    let out = ConversationService::serialize_artifact_for_context(&a);
    assert!(!out.contains("[File:"));
}

#[test]
fn serialize_artifact_for_context_empty_parts() {
    let a = make_artifact(vec![], Some("empty"), "summary");
    let out = ConversationService::serialize_artifact_for_context(&a);
    assert_eq!(out, "[Artifact: empty (type: summary)]\n");
}
