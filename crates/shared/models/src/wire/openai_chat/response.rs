//! `OpenAI` Chat Completions buffered-response parsing into the canonical
//! model.

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
}

#[derive(Debug, Default, Deserialize)]
struct ChatPromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

impl ChatUsage {
    const fn into_canonical(self) -> CanonicalUsage {
        CanonicalUsage {
            input_tokens: self.prompt_tokens,
            output_tokens: self.completion_tokens,
            cache_read_tokens: self.prompt_tokens_details.cached_tokens,
            cache_creation_tokens: 0,
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
    }
}

fn collect_message_content(msg: ChatMessage, content: &mut Vec<CanonicalContent>) {
    if let Some(text) = msg.content {
        if !text.is_empty() {
            content.push(CanonicalContent::Text(text));
        }
    }
    for tc in msg.tool_calls {
        let args = if tc.function.arguments.is_empty() {
            "{}"
        } else {
            &tc.function.arguments
        };
        // Tool-call arguments are a user-defined schema instance; the canonical
        // model carries them as an opaque JSON value, not a typed shape.
        let input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
        content.push(CanonicalContent::ToolUse {
            id: tc.id,
            name: tc.function.name,
            input,
        });
    }
}
