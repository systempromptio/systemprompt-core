//! Renders a [`CanonicalRequest`] into a Gemini `generateContent` body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::wire::{
    GeminiContent, GeminiEmpty, GeminiFunctionCall, GeminiFunctionCallingConfig,
    GeminiFunctionDeclaration, GeminiFunctionResponse, GeminiGenerationConfig, GeminiInlineData,
    GeminiPart, GeminiRequest, GeminiSystemInstruction, GeminiTool, GeminiToolConfig,
};
use crate::schema::SchemaSanitizer;
use crate::services::WireProtocol;
use crate::services::ai::ModelLimits;
use crate::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource,
    ResponseFormat, Role,
};

#[must_use]
// JSON: Gemini `generateContent` request body; upstream JSON is the contract.
pub fn build_request_body(request: &CanonicalRequest, limits: Option<ModelLimits>) -> Value {
    let body = GeminiRequest {
        contents: contents(request),
        system_instruction: request.system.as_ref().map(|s| GeminiSystemInstruction {
            parts: vec![plain_text_part(s.clone())],
        }),
        generation_config: Some(generation_config(request, limits)),
        tools: tools(request),
        tool_config: request.tool_choice.as_ref().map(tool_config),
    };
    serde_json::to_value(&body).unwrap_or(Value::Null)
}

fn generation_config(
    request: &CanonicalRequest,
    limits: Option<ModelLimits>,
) -> GeminiGenerationConfig {
    let (response_mime_type, response_schema) = match &request.response_format {
        // Why: Gemini's `responseSchema` uses an OpenAPI subset: no `additionalProperties`
        // or type lists, and nullability is a flag.
        Some(ResponseFormat::JsonSchema { schema, .. }) => {
            let sanitizer = SchemaSanitizer::new(WireProtocol::Gemini.schema_capabilities());
            (
                Some("application/json".to_owned()),
                Some(sanitizer.sanitize(schema.clone())),
            )
        },
        Some(ResponseFormat::JsonObject) => (Some("application/json".to_owned()), None),
        None => (None, None),
    };
    let (thinking_config, max_output_tokens) = super::thinking::thinking_config(request, limits);
    GeminiGenerationConfig {
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: request.top_k,
        max_output_tokens: Some(max_output_tokens),
        stop_sequences: if request.stop_sequences.is_empty() {
            None
        } else {
            Some(request.stop_sequences.clone())
        },
        response_mime_type,
        response_schema,
        thinking_config,
    }
}

fn tools(request: &CanonicalRequest) -> Option<Vec<GeminiTool>> {
    let mut tools: Vec<GeminiTool> = Vec::new();
    if !request.tools.is_empty() {
        let sanitizer = SchemaSanitizer::new(WireProtocol::Gemini.schema_capabilities());
        let declarations = request
            .tools
            .iter()
            .map(|t| GeminiFunctionDeclaration {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: sanitizer.sanitize(t.input_schema.clone()),
            })
            .collect();
        tools.push(GeminiTool::Functions {
            function_declarations: declarations,
        });
    }
    if let Some(search) = &request.search {
        tools.push(GeminiTool::GoogleSearch {
            google_search: GeminiEmpty {},
        });
        if !search.urls.is_empty() {
            tools.push(GeminiTool::UrlContext {
                url_context: GeminiEmpty {},
            });
        }
    }
    if request.code_execution {
        tools.push(GeminiTool::CodeExecution {
            code_execution: GeminiEmpty {},
        });
    }
    (!tools.is_empty()).then_some(tools)
}

fn tool_config(choice: &CanonicalToolChoice) -> GeminiToolConfig {
    let (mode, allowed) = match choice {
        CanonicalToolChoice::Auto => ("AUTO", None),
        CanonicalToolChoice::None => ("NONE", None),
        CanonicalToolChoice::Any | CanonicalToolChoice::Required => ("ANY", None),
        CanonicalToolChoice::Tool(name) => ("ANY", Some(vec![name.clone()])),
    };
    GeminiToolConfig {
        function_calling_config: GeminiFunctionCallingConfig {
            mode,
            allowed_function_names: allowed,
        },
    }
}

// Why: Gemini has no system role inside `contents`, and a mid-history system
// message is harness context the model must see — Claude Code sends its
// environment block and the available-skills listing that way. It travels as
// user text, folded into the neighbouring user turn so the history keeps
// alternating; model turns are left as sent so thought replay keeps its shape.
fn contents(request: &CanonicalRequest) -> Vec<GeminiContent> {
    let call_names = tool_call_names(request);
    let mut contents: Vec<GeminiContent> = Vec::new();
    for msg in &request.messages {
        let Some(content) = message_to_content(msg, &call_names) else {
            continue;
        };
        match contents.last_mut() {
            Some(last) if last.role == "user" && content.role == "user" => {
                last.parts.extend(content.parts);
            },
            _ => contents.push(content),
        }
    }
    for content in &mut contents {
        if content.role == "user" {
            fold_text_into_function_responses(&mut content.parts);
        }
    }
    contents
}

// Why: a user turn that answers function calls may also carry text — Claude
// Code delivers a skill as a `tool_result` saying "Launching skill" followed
// by a text block holding the skill body. Gemini 3.5 answers such a turn with
// an empty STOP every time the text stands beside the function responses,
// and follows it once the text is inside the function result, so the texts
// join the responses in order; any surplus lands on the last one.
fn fold_text_into_function_responses(parts: &mut Vec<GeminiPart>) {
    let has_response = parts
        .iter()
        .any(|part| matches!(part, GeminiPart::FunctionResponse { .. }));
    if !has_response {
        return;
    }
    let mut texts: Vec<String> = Vec::new();
    let mut kept: Vec<GeminiPart> = Vec::with_capacity(parts.len());
    for part in parts.drain(..) {
        match part {
            GeminiPart::Text {
                text,
                thought: None | Some(false),
                ..
            } => texts.push(text),
            other => kept.push(other),
        }
    }
    let mut responses: Vec<&mut GeminiFunctionResponse> = kept
        .iter_mut()
        .filter_map(|part| match part {
            GeminiPart::FunctionResponse { function_response } => Some(function_response),
            _ => None,
        })
        .collect();
    let last = responses.len() - 1;
    for (i, text) in texts.into_iter().enumerate() {
        append_context(responses[i.min(last)], &text);
    }
    *parts = kept;
}

fn append_context(response: &mut GeminiFunctionResponse, text: &str) {
    if let Some(Value::String(result)) = response.response.get_mut("result") {
        result.push_str("\n\n");
        result.push_str(text);
        return;
    }
    if let Some(map) = response.response.as_object_mut() {
        match map.get_mut("context") {
            Some(Value::String(existing)) => {
                existing.push_str("\n\n");
                existing.push_str(text);
            },
            _ => {
                map.insert("context".to_owned(), Value::String(text.to_owned()));
            },
        }
    }
}

// Why: Gemini requires `functionResponse.name` to be the declared function
// name.
fn tool_call_names(request: &CanonicalRequest) -> HashMap<&str, &str> {
    let mut names = HashMap::new();
    for msg in &request.messages {
        for part in &msg.content {
            if let CanonicalContent::ToolUse { id, name, .. } = part {
                names.insert(id.as_str(), name.as_str());
            }
        }
    }
    names
}

fn message_to_content(
    msg: &CanonicalMessage,
    call_names: &HashMap<&str, &str>,
) -> Option<GeminiContent> {
    let role = match msg.role {
        Role::Assistant => "model",
        Role::User | Role::Tool | Role::System => "user",
    };
    let parts: Vec<GeminiPart> = msg
        .content
        .iter()
        .map(|part| content_to_part(part, call_names))
        .collect();
    if parts.is_empty() {
        return None;
    }
    Some(GeminiContent {
        role: role.to_owned(),
        parts,
    })
}

fn content_to_part(part: &CanonicalContent, call_names: &HashMap<&str, &str>) -> GeminiPart {
    match part {
        CanonicalContent::Text(t) => plain_text_part(t.clone()),
        CanonicalContent::Image(src) => image_part(src),
        CanonicalContent::ToolUse {
            name,
            input,
            signature,
            ..
        } => GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: name.clone(),
                args: input.clone(),
            },
            thought_signature: signature.clone(),
        },
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured_content,
            ..
        } => tool_result_part(
            call_names
                .get(tool_use_id.as_str())
                .copied()
                .unwrap_or(tool_use_id),
            content,
            *is_error,
            structured_content.as_ref(),
        ),
        CanonicalContent::Thinking {
            text, signature, ..
        } => GeminiPart::Text {
            text: text.clone(),
            thought: Some(true),
            thought_signature: signature.clone(),
        },
    }
}

const fn plain_text_part(text: String) -> GeminiPart {
    GeminiPart::Text {
        text,
        thought: None,
        thought_signature: None,
    }
}

fn image_part(src: &ImageSource) -> GeminiPart {
    match src {
        ImageSource::Base64 {
            media_type, data, ..
        } => GeminiPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: media_type.clone(),
                data: data.clone(),
            },
        },
        // Why: Gemini image parts require inline base64 data or a Files API handle,
        // not an arbitrary image URL.
        ImageSource::Url { url, .. } => {
            tracing::warn!(
                url = %url,
                "Gemini accepts only inline base64 image data; the image URL was \
                 downgraded to a plain text part and will not be seen as an image"
            );
            plain_text_part(url.clone())
        },
    }
}

fn tool_result_part(
    function_name: &str,
    content: &[CanonicalContent],
    is_error: bool,
    structured_content: Option<&Value>,
) -> GeminiPart {
    let response = if is_error {
        json!({ "error": flatten_text(content) })
    } else if let Some(sc) = structured_content {
        json!({ "result": sc })
    } else {
        json!({ "result": flatten_text(content) })
    };
    GeminiPart::FunctionResponse {
        function_response: GeminiFunctionResponse {
            name: function_name.to_owned(),
            response,
        },
    }
}

fn flatten_text(parts: &[CanonicalContent]) -> String {
    let mut out = String::new();
    for p in parts {
        if let CanonicalContent::Text(t) = p {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}
