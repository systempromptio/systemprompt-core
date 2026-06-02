//! Per-dialect wire-codec tests, one submodule per provider wire.
//!
//! Each submodule pins the request-build, response-parse, and SSE behaviour of
//! one codec, including the documented idiosyncrasies that differ between
//! dialects (output-token field name, tool nesting, role mapping, schema
//! keyword handling, thought signatures). Shared request fixtures live here.

use serde_json::json;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, ImageSource, Role,
};

mod anthropic;
mod gemini;
mod openai_chat;
mod openai_responses;

fn tool_with_unsupported_keywords() -> CanonicalTool {
    CanonicalTool {
        name: "do_thing".to_owned(),
        description: Some("d".to_owned()),
        input_schema: json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": false,
            "propertyNames": {"pattern": "^[a-z]+$"},
            "properties": {
                "count": {"type": "integer", "exclusiveMinimum": 0}
            }
        }),
    }
}

fn plain_tool() -> CanonicalTool {
    CanonicalTool {
        name: "lookup".to_owned(),
        description: Some("look something up".to_owned()),
        input_schema: json!({
            "type": "object",
            "properties": {"q": {"type": "string"}}
        }),
    }
}

fn tool_use(signature: Option<&str>) -> CanonicalContent {
    CanonicalContent::ToolUse {
        id: "call_1".to_owned(),
        name: "lookup".to_owned(),
        input: json!({"q": "rust"}),
        signature: signature.map(str::to_owned),
    }
}

fn image_url(url: &str) -> CanonicalContent {
    CanonicalContent::Image(ImageSource::Url {
        url: url.to_owned(),
        detail: None,
    })
}

fn user_message(content: Vec<CanonicalContent>) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::User,
        content,
    }
}

fn base_request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".to_owned(),
        system: None,
        messages: vec![user_message(vec![CanonicalContent::Text("hi".to_owned())])],
        max_tokens: 32,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
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
    }
}
