#![no_main]
//! Arbitrary client request bodies, on any inbound wire, through parse →
//! normalise → every upstream encoder: nothing may panic, and what reaches
//! an Anthropic or OpenAI Chat upstream must already be strictly paired.

use libfuzzer_sys::fuzz_target;
use serde_json::Value;
use systemprompt_gateway::protocol::inbound::{anthropic_messages, openai_chat, openai_responses};
use systemprompt_wire::canonical::CanonicalContent;
use systemprompt_wire::{anthropic, gemini, openai_responses as responses_wire};

fuzz_target!(|data: &[u8]| {
    let Some((selector, body)) = data.split_first() else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return;
    };
    let parsed = match selector % 3 {
        0 => openai_chat::parse::parse(&value),
        1 => openai_responses::parse::parse(&value),
        _ => anthropic_messages::parse::parse(&value),
    };
    let Ok(request) = parsed else {
        return;
    };
    // Server tools (web search, tool search) pair their own blocks upstream,
    // so a history carrying them is not held to client-side pairing.
    let server_tools = request.messages.iter().any(|m| {
        m.content
            .iter()
            .any(|p| matches!(p, CanonicalContent::AnthropicToolBlock { .. }))
    });
    let to_anthropic = anthropic::build_request_body(&request, "m", None);
    assert!(
        server_tools || anthropic::history_is_strict(&to_anthropic["messages"]),
        "anthropic upstream body is not strictly paired: {to_anthropic}"
    );
    let to_chat = systemprompt_wire::openai_chat::build_request_body(&request, "m", None);
    assert!(
        systemprompt_wire::openai_chat::history_is_strict(&to_chat["messages"]),
        "openai chat upstream body is not strictly paired: {to_chat}"
    );
    let _ = responses_wire::build_request_body(&request, "m", None);
    let _ = gemini::build_request_body(&request, None);
});
