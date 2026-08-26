//! Unit tests for `CanonicalRequest` helpers — `flatten_parts`,
//! `flatten_message_text`, and `derived_gateway_conversation_id`.

use serde_json::json;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};

fn req_with(messages: Vec<CanonicalMessage>, system: Option<&str>) -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: system.map(str::to_owned),
        messages,
        max_tokens: 10,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

fn user(text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::Text(text.into())],
    }
}

fn assistant(text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::Text(text.into())],
    }
}

#[test]
fn flatten_parts_names_system_and_each_message() {
    let r = req_with(vec![user("hello"), assistant("hi")], Some("be helpful"));
    assert_eq!(
        r.flatten_parts(),
        vec![
            ("system".to_owned(), "be helpful".to_owned()),
            ("messages[0].user".to_owned(), "hello".to_owned()),
            ("messages[1].assistant".to_owned(), "hi".to_owned()),
        ]
    );
}

#[test]
fn flatten_parts_skips_empty_system() {
    let r = req_with(vec![user("hello")], Some(""));
    assert_eq!(
        r.flatten_parts(),
        vec![("messages[0].user".to_owned(), "hello".to_owned())]
    );
}

#[test]
fn flatten_parts_handles_no_system() {
    let r = req_with(vec![user("hello")], None);
    assert_eq!(
        r.flatten_parts(),
        vec![("messages[0].user".to_owned(), "hello".to_owned())]
    );
}

#[test]
fn flatten_parts_renders_tool_use_as_bracketed() {
    let mut r = req_with(vec![user("call it")], None);
    r.messages.push(CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::ToolUse {
            id: "tu_1".into(),
            name: "search".into(),
            input: json!({"q": "rust"}),
            signature: None,
        }],
    });
    let s = r
        .flatten_parts()
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(s.contains("call it"));
    assert!(s.contains("[tool_use:search"));
    assert!(s.contains("rust"));
}

#[test]
fn flatten_parts_skips_images() {
    use systemprompt_api::services::gateway::protocol::canonical::ImageSource;
    let r = req_with(
        vec![CanonicalMessage {
            role: Role::User,
            content: vec![
                CanonicalContent::Text("look".into()),
                CanonicalContent::Image(ImageSource::Url {
                    url: "https://x".into(),
                    detail: None,
                }),
            ],
        }],
        None,
    );
    assert_eq!(
        r.flatten_parts(),
        vec![("messages[0].user".to_owned(), "look".to_owned())]
    );
}

#[test]
fn flatten_message_text_filters_by_role() {
    let r = req_with(vec![user("u1"), assistant("a1"), user("u2")], None);
    assert_eq!(
        r.flatten_message_text(Role::User).as_deref(),
        Some("u1\nu2")
    );
    assert_eq!(
        r.flatten_message_text(Role::Assistant).as_deref(),
        Some("a1")
    );
    assert!(r.flatten_message_text(Role::System).is_none());
}

#[test]
fn derived_gateway_conversation_id_none_when_no_messages() {
    let r = req_with(vec![], None);
    assert!(r.derived_gateway_conversation_id().is_none());
}

#[test]
fn derived_gateway_conversation_id_is_deterministic() {
    let a = req_with(vec![user("hello world")], Some("you are helpful"));
    let b = req_with(vec![user("hello world")], Some("you are helpful"));
    let ida = a.derived_gateway_conversation_id().unwrap();
    let idb = b.derived_gateway_conversation_id().unwrap();
    assert_eq!(ida.as_str(), idb.as_str());
}

#[test]
fn derived_gateway_conversation_id_differs_for_different_content() {
    let a = req_with(vec![user("hello")], None);
    let b = req_with(vec![user("world")], None);
    let ida = a.derived_gateway_conversation_id().unwrap();
    let idb = b.derived_gateway_conversation_id().unwrap();
    assert_ne!(ida.as_str(), idb.as_str());
}
