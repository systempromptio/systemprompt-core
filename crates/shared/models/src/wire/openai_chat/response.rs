//! `OpenAI` Chat Completions buffered-response parsing into the canonical
//! model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::wire::canonical::{
    CanonicalContent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

#[derive(Debug, Default, Deserialize)]
struct ChatCompletion {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<ChatUsage>,
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: ChatPromptTokensDetails,
    #[serde(default)]
    completion_tokens_details: ChatCompletionTokensDetails,
}

#[derive(Debug, Default, Deserialize)]
struct ChatPromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

// Why: emitted by OpenAI's reasoning models and by the OpenAI-compatible
// providers that copy the contract (DeepSeek, Qwen, Moonshot). The count is a
// breakdown of completion_tokens, not an addition to it.
#[derive(Debug, Default, Deserialize)]
struct ChatCompletionTokensDetails {
    #[serde(default)]
    reasoning_tokens: u32,
}

impl ChatUsage {
    const fn into_canonical(self) -> CanonicalUsage {
        CanonicalUsage {
            input_tokens: self.prompt_tokens,
            output_tokens: self.completion_tokens,
            cache_read_tokens: self.prompt_tokens_details.cached_tokens,
            cache_creation_tokens: 0,
            reasoning_tokens: self.completion_tokens_details.reasoning_tokens,
            total_tokens: self.total_tokens,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    message: Option<ChatMessage>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
    // Why: the OpenAI chat contract has no reasoning field, but every
    // OpenAI-compatible provider that emits thinking (DeepSeek, Qwen,
    // Moonshot) puts it here; `alias` accepts the shorter spelling some of
    // them use. Without it a thinking model's reasoning is discarded on
    // arrival and cannot be replayed on the next turn.
    #[serde(default, alias = "reasoning")]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ChatToolCall>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatToolCall {
    #[serde(default)]
    id: String,
    #[serde(default)]
    function: ChatFunction,
}

#[derive(Debug, Default, Deserialize)]
struct ChatFunction {
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: String,
}

pub fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let resp = ChatCompletion::deserialize(value).unwrap_or_default();
    let id = resp
        .id
        .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple()));
    let model = resp.model.unwrap_or_else(|| fallback_model.to_owned());
    let usage = resp
        .usage
        .map(ChatUsage::into_canonical)
        .unwrap_or_default();

    let mut content: Vec<CanonicalContent> = Vec::new();
    let mut stop_reason = None;
    let mut raw_finish_reason = None;
    if let Some(choice) = resp.choices.into_iter().next() {
        raw_finish_reason.clone_from(&choice.finish_reason);
        stop_reason = choice
            .finish_reason
            .as_deref()
            .map(CanonicalStopReason::from_openai);
        if let Some(msg) = choice.message {
            collect_message_content(msg, &mut content);
        }
        // Why: the contract says a turn carrying tool_calls finishes with
        // "tool_calls", but several OpenAI-compatible upstreams send a plain
        // "stop" beside a fully-formed tool_calls array. Relayed as "stop" the
        // client ends the turn and never runs the call, and the drop is
        // silent -- it looks exactly like the model declining to use tools.
        let has_tool_use = content
            .iter()
            .any(|c| matches!(c, CanonicalContent::ToolUse { .. }));
        stop_reason = stop_reason.map(|r| r.with_tool_use(has_tool_use));
    }

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
        grounding: None,
        code_execution: None,
        raw_finish_reason,
        ..Default::default()
    }
}

fn collect_message_content(msg: ChatMessage, content: &mut Vec<CanonicalContent>) {
    if let Some(reasoning) = msg.reasoning_content
        && !reasoning.is_empty()
    {
        content.push(CanonicalContent::Thinking {
            text: reasoning,
            signature: None,
            id: None,
            encrypted_content: None,
        });
    }
    if let Some(text) = msg.content
        && !text.is_empty()
    {
        content.push(CanonicalContent::Text(text));
    }
    for tc in msg.tool_calls {
        let args = if tc.function.arguments.is_empty() {
            "{}"
        } else {
            &tc.function.arguments
        };
        // JSON: Tool-call arguments are a user-defined schema instance; the canonical
        // model carries them as an opaque JSON value, not a typed shape.
        let input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
        content.push(CanonicalContent::ToolUse {
            id: tc.id,
            name: tc.function.name,
            input,
            signature: None,
        });
    }
}
