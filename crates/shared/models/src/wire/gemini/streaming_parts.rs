//! Part emitters for the Gemini SSE codec: text, thought and function-call
//! parts become canonical content-block events, each kind on its own block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use uuid::Uuid;

use super::streaming::StreamState;
use super::wire::GeminiPart;
use crate::wire::canonical::{CanonicalEvent, ContentBlockKind};

pub(super) fn close_text(
    state: &mut StreamState,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if let Some(index) = state.text_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
}

pub(super) fn close_thinking(
    state: &mut StreamState,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    if let Some(index) = state.thinking_block.take() {
        events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
    }
}

pub(super) fn emit_part(
    state: &mut StreamState,
    part: &GeminiPart,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    match part {
        GeminiPart::Text {
            text,
            thought: Some(true),
            thought_signature,
        } => emit_thought(state, text, thought_signature.clone(), events),
        GeminiPart::Text { text, .. } if !text.is_empty() => {
            state.emitted_part = true;
            emit_text(state, text, events);
        },
        GeminiPart::FunctionCall {
            function_call,
            thought_signature,
        } => {
            close_thinking(state, events);
            state.emitted_tool_use = true;
            state.emitted_part = true;
            emit_tool_use(
                state,
                &function_call.name,
                &function_call.args,
                thought_signature.clone(),
                events,
            );
        },
        _ => {},
    }
}

fn emit_text(
    state: &mut StreamState,
    text: &str,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    close_thinking(state, events);
    let index = if let Some(index) = state.text_block {
        index
    } else {
        let index = state.next_index;
        state.next_index += 1;
        state.text_block = Some(index);
        events.push(Ok(CanonicalEvent::ContentBlockStart {
            index,
            block: ContentBlockKind::Text,
        }));
        index
    };
    events.push(Ok(CanonicalEvent::TextDelta {
        index,
        text: text.to_owned(),
    }));
}

fn emit_thought(
    state: &mut StreamState,
    text: &str,
    signature: Option<String>,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let index = if let Some(index) = state.thinking_block {
        index
    } else {
        let index = state.next_index;
        state.next_index += 1;
        state.thinking_block = Some(index);
        events.push(Ok(CanonicalEvent::ContentBlockStart {
            index,
            block: ContentBlockKind::Thinking {
                id: None,
                signature: None,
            },
        }));
        index
    };
    if !text.is_empty() {
        events.push(Ok(CanonicalEvent::ThinkingDelta {
            index,
            text: text.to_owned(),
        }));
    }
    if let Some(signature) = signature {
        events.push(Ok(CanonicalEvent::SignatureDelta { index, signature }));
    }
}

fn emit_tool_use(
    state: &mut StreamState,
    name: &str,
    // JSON: Gemini `functionCall.args` is the tool's own JSON argument object.
    args: &Value,
    signature: Option<String>,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let index = state.next_index;
    state.next_index += 1;
    events.push(Ok(CanonicalEvent::ContentBlockStart {
        index,
        block: ContentBlockKind::ToolUse {
            id: format!("call_{}", Uuid::new_v4().simple()),
            name: name.to_owned(),
            signature,
        },
    }));
    events.push(Ok(CanonicalEvent::ToolUseDelta {
        index,
        partial_json: args.to_string(),
    }));
    events.push(Ok(CanonicalEvent::ContentBlockStop { index }));
}
