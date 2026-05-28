//! Unit tests for MessageValidationService pure helpers.
//!
//! Target: crates/domain/agent/src/services/a2a_server/processing/
//! message_validation.rs

use systemprompt_agent::models::a2a::{DataPart, Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::processing::MessageValidationService;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

fn msg_with_parts(parts: Vec<Part>, task_id: Option<TaskId>) -> Message {
    Message {
        role: MessageRole::User,
        parts,
        message_id: MessageId::generate(),
        task_id,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn text_part(s: &str) -> Part {
    Part::Text(TextPart {
        text: s.to_string(),
    })
}

fn data_only_part() -> Part {
    Part::Data(DataPart {
        data: serde_json::Map::new(),
    })
}

#[test]
fn validate_message_format_accepts_text_part() {
    let m = msg_with_parts(vec![text_part("hi")], None);
    assert!(MessageValidationService::validate_message_format(&m).is_ok());
}

#[test]
fn validate_message_format_accepts_text_among_others() {
    let m = msg_with_parts(vec![data_only_part(), text_part("hi")], None);
    assert!(MessageValidationService::validate_message_format(&m).is_ok());
}

#[test]
fn validate_message_format_rejects_no_text() {
    let m = msg_with_parts(vec![data_only_part()], None);
    let err = MessageValidationService::validate_message_format(&m).expect_err("no text");
    assert!(err.to_string().contains("No text content"));
}

#[test]
fn validate_message_format_rejects_empty_parts() {
    let m = msg_with_parts(vec![], None);
    assert!(MessageValidationService::validate_message_format(&m).is_err());
}

#[test]
fn determine_task_id_uses_existing() {
    let tid = TaskId::new("known-task-id");
    let m = msg_with_parts(vec![text_part("x")], Some(tid.clone()));
    assert_eq!(MessageValidationService::determine_task_id(&m), tid);
}

#[test]
fn determine_task_id_generates_when_missing() {
    let m = msg_with_parts(vec![text_part("x")], None);
    let id = MessageValidationService::determine_task_id(&m);
    assert!(!id.as_str().is_empty());
    // Generated form is uuidv4.
    assert!(id.as_str().contains('-'));
}

#[test]
fn determine_task_id_generates_unique_each_call() {
    let m = msg_with_parts(vec![text_part("x")], None);
    let a = MessageValidationService::determine_task_id(&m);
    let b = MessageValidationService::determine_task_id(&m);
    assert_ne!(a, b);
}
