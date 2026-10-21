//! What an `OpenAI` Chat client (`OpenCode`, Cursor) receives when the
//! upstream reply opens with thinking and then makes parallel tool calls:
//! the reasoning as `reasoning_content`, and streamed `tool_calls` numbered
//! from 0 per choice even though the canonical block index counts the
//! thinking block too.

use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
    ContentBlockKind,
};
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::stream_tap::accumulator::{
    TapState, accumulate_event, with_tool_ordinal,
};

fn data(frame: &Bytes) -> Value {
    let s = std::str::from_utf8(frame).expect("utf8");
    let line = s
        .lines()
        .find_map(|l| l.strip_prefix("data: "))
        .expect("data line");
    serde_json::from_str(line).expect("chunk json")
}

fn tool(id: &str, file: &str) -> CanonicalContent {
    CanonicalContent::ToolUse {
        id: id.to_owned(),
        name: "read".to_owned(),
        input: json!({ "filePath": file }),
        signature: None,
        cache_control: None,
    }
}

fn thinking_then_two_calls() -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "claude".into(),
        content: vec![
            CanonicalContent::Thinking {
                text: "read both".to_owned(),
                signature: Some("sig".to_owned()),
                id: None,
                encrypted_content: None,
            },
            tool("toolu_1", "a.rs"),
            tool("toolu_2", "b.rs"),
        ],
        stop_reason: Some(CanonicalStopReason::ToolUse),
        usage: CanonicalUsage::default(),
        ..Default::default()
    }
}

#[test]
fn buffered_chat_reply_carries_thinking_as_reasoning_content() {
    let v: Value =
        serde_json::from_slice(&OpenAiChatInbound.render_response(&thinking_then_two_calls()))
            .expect("json");
    let message = &v["choices"][0]["message"];
    assert_eq!(message["reasoning_content"], "read both");
    assert_eq!(message["tool_calls"][0]["id"], "toolu_1");
    assert_eq!(message["tool_calls"][1]["id"], "toolu_2");
}

#[test]
fn buffered_chat_reply_without_thinking_has_no_reasoning_content() {
    let mut response = thinking_then_two_calls();
    response.content.remove(0);
    let v: Value =
        serde_json::from_slice(&OpenAiChatInbound.render_response(&response)).expect("json");
    assert!(
        v["choices"][0]["message"]
            .get("reasoning_content")
            .is_none()
    );
}

#[test]
fn streamed_thinking_delta_is_a_reasoning_content_chunk() {
    let frame = OpenAiChatInbound
        .render_event(
            &CanonicalEvent::ThinkingDelta {
                index: 0,
                text: "hmm".into(),
            },
            "m",
        )
        .expect("frame");
    assert_eq!(
        data(&frame)["choices"][0]["delta"]["reasoning_content"],
        "hmm"
    );
}

fn streamed_events() -> Vec<CanonicalEvent> {
    let mut events = vec![
        CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Thinking {
                id: None,
                signature: None,
            },
        },
        CanonicalEvent::ThinkingDelta {
            index: 0,
            text: "read both".into(),
        },
    ];
    for (index, id) in [(1, "toolu_1"), (2, "toolu_2")] {
        events.push(CanonicalEvent::ContentBlockStart {
            index,
            block: ContentBlockKind::ToolUse {
                id: id.into(),
                name: "read".into(),
                signature: None,
            },
        });
        events.push(CanonicalEvent::ToolUseDelta {
            index,
            partial_json: "{}".into(),
        });
    }
    events
}

// The stream tap folds each event into its state, then renumbers it for an
// adapter that indexes tool calls by ordinal, exactly in this order.
fn tool_call_indexes(adapter: &dyn InboundAdapter) -> Vec<u64> {
    let mut state = TapState::default();
    let mut indexes = Vec::new();
    for event in streamed_events() {
        accumulate_event(&mut state, &event);
        let event = if adapter.indexes_tool_calls_by_ordinal() {
            with_tool_ordinal(&state, &event).unwrap_or(event)
        } else {
            event
        };
        if let Some(frame) = adapter.render_event(&event, "m")
            && let Some(index) =
                data(&frame)["choices"][0]["delta"]["tool_calls"][0]["index"].as_u64()
        {
            indexes.push(index);
        }
    }
    indexes
}

#[test]
fn streamed_chat_tool_calls_are_indexed_from_zero_after_thinking() {
    assert_eq!(tool_call_indexes(&OpenAiChatInbound), [0, 0, 1, 1]);
}

#[test]
fn anthropic_clients_keep_the_canonical_block_index() {
    assert!(!AnthropicMessagesInbound.indexes_tool_calls_by_ordinal());
}

#[test]
fn renumbering_leaves_non_tool_events_alone() {
    let state = TapState::default();
    let text = CanonicalEvent::TextDelta {
        index: 3,
        text: "x".into(),
    };
    assert!(with_tool_ordinal(&state, &text).is_none());
}

// OpenCode fails a turn whose usage does not add up ("DecimalError") when a
// cache-hitting upstream (DeepSeek) reports most of the prompt as cached.
#[test]
fn chat_usage_counts_cached_tokens_inside_prompt_tokens() {
    let mut response = thinking_then_two_calls();
    response.usage = CanonicalUsage {
        input_tokens: 144,
        output_tokens: 22,
        cache_read_tokens: 9856,
        cache_creation_tokens: 0,
        reasoning_tokens: 0,
        total_tokens: 0,
    };
    let v: Value =
        serde_json::from_slice(&OpenAiChatInbound.render_response(&response)).expect("json");
    assert_eq!(v["usage"]["prompt_tokens"], 10_000);
    assert_eq!(v["usage"]["prompt_tokens_details"]["cached_tokens"], 9856);
    assert_eq!(v["usage"]["total_tokens"], 10_022);
}
