//! `is_new_user_turn` decides whether a request opens a user turn (recall
//! fresh) or continues one through a tool round trip (reuse the turn's
//! block).

use bytes::Bytes;
use serde_json::json;
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, Role, is_new_user_turn,
};
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;

fn text(text: &str) -> CanonicalContent {
    CanonicalContent::Text {
        text: text.to_owned(),
        cache_control: None,
    }
}

fn tool_result() -> CanonicalContent {
    CanonicalContent::ToolResult {
        tool_use_id: "tu_1".to_owned(),
        content: vec![text("42")],
        is_error: false,
        structured_content: None,
        meta: None,
        cache_control: None,
    }
}

fn message(role: Role, content: Vec<CanonicalContent>) -> CanonicalMessage {
    CanonicalMessage { role, content }
}

#[test]
fn a_user_text_turn_is_new() {
    assert!(is_new_user_turn(&[message(Role::User, vec![text("hi")])]));
}

#[test]
fn a_user_turn_carrying_only_a_tool_result_is_not_new() {
    assert!(!is_new_user_turn(&[message(
        Role::User,
        vec![tool_result()]
    )]));
}

#[test]
fn a_user_turn_mixing_text_and_a_tool_result_is_not_new() {
    assert!(!is_new_user_turn(&[message(
        Role::User,
        vec![tool_result(), text("and also")]
    )]));
}

#[test]
fn a_trailing_assistant_turn_is_not_new() {
    assert!(!is_new_user_turn(&[
        message(Role::User, vec![text("hi")]),
        message(Role::Assistant, vec![text("hello")]),
    ]));
}

#[test]
fn an_empty_history_is_not_new() {
    assert!(!is_new_user_turn(&[]));
    assert!(!is_new_user_turn(&[message(Role::User, Vec::new())]));
}

#[test]
fn a_trailing_responses_developer_item_is_not_new() {
    let body = json!({
        "model": "gpt-x",
        "input": [
            { "type": "message", "role": "user", "content": "hi" },
            { "type": "message", "role": "developer", "content": "be brief" }
        ]
    });
    let bytes = Bytes::from(serde_json::to_vec(&body).expect("serialize"));
    let request = OpenAiResponsesInbound
        .parse_request(&bytes)
        .expect("a Responses body parses");

    assert!(!is_new_user_turn(&request.messages));
}
