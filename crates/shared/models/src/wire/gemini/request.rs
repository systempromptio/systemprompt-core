//! Renders a [`CanonicalRequest`] into a Gemini `generateContent` body.

use serde_json::{Value, json};

use super::wire::{
    GeminiContent, GeminiEmpty, GeminiFunctionCall, GeminiFunctionCallingConfig,
    GeminiFunctionDeclaration, GeminiFunctionResponse, GeminiGenerationConfig, GeminiInlineData,
    GeminiPart, GeminiRequest, GeminiSystemInstruction, GeminiThinkingConfig, GeminiTool,
    GeminiToolConfig,
};
use crate::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource,
    ResponseFormat, Role,
};

#[must_use]
pub fn build_request_body(request: &CanonicalRequest) -> Value {
    let body = GeminiRequest {
        contents: contents(request),
        system_instruction: request.system.as_ref().map(|s| GeminiSystemInstruction {
            parts: vec![GeminiPart::Text { text: s.clone() }],
        }),
        generation_config: Some(generation_config(request)),
        tools: tools(request),
        tool_config: request.tool_choice.as_ref().map(tool_config),
    };
    serde_json::to_value(&body).unwrap_or(Value::Null)
}

fn generation_config(request: &CanonicalRequest) -> GeminiGenerationConfig {
    let (response_mime_type, response_schema) = match &request.response_format {
        Some(ResponseFormat::JsonSchema { schema, .. }) => {
            (Some("application/json".to_owned()), Some(schema.clone()))
        },
        Some(ResponseFormat::JsonObject) => (Some("application/json".to_owned()), None),
        None => (None, None),
    };
    GeminiGenerationConfig {
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: request.top_k,
        max_output_tokens: Some(request.max_tokens),
        stop_sequences: if request.stop_sequences.is_empty() {
            None
        } else {
            Some(request.stop_sequences.clone())
        },
        response_mime_type,
        response_schema,
        thinking_config: thinking_config(request),
    }
}

fn thinking_config(request: &CanonicalRequest) -> Option<GeminiThinkingConfig> {
    let thinking = request.thinking?;
    if !thinking.enabled {
        return None;
    }
    Some(GeminiThinkingConfig {
        thinking_budget: thinking.budget_tokens,
        include_thoughts: None,
    })
}

fn tools(request: &CanonicalRequest) -> Option<Vec<GeminiTool>> {
    let mut tools: Vec<GeminiTool> = Vec::new();
    if !request.tools.is_empty() {
        let declarations = request
            .tools
            .iter()
            .map(|t| GeminiFunctionDeclaration {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
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

fn contents(request: &CanonicalRequest) -> Vec<GeminiContent> {
    request
        .messages
        .iter()
        .filter_map(message_to_content)
        .collect()
}

fn message_to_content(msg: &CanonicalMessage) -> Option<GeminiContent> {
    let role = match msg.role {
        Role::System => return None,
        Role::Assistant => "model",
        Role::User | Role::Tool => "user",
    };
    let parts: Vec<GeminiPart> = msg.content.iter().filter_map(content_to_part).collect();
    if parts.is_empty() {
        return None;
    }
    Some(GeminiContent {
        role: role.to_owned(),
        parts,
    })
}

fn content_to_part(part: &CanonicalContent) -> Option<GeminiPart> {
    match part {
        CanonicalContent::Text(t) => Some(GeminiPart::Text { text: t.clone() }),
        CanonicalContent::Image(src) => Some(image_part(src)),
        CanonicalContent::ToolUse { name, input, .. } => Some(GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: name.clone(),
                args: input.clone(),
            },
        }),
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Some(tool_result_part(tool_use_id, content, *is_error)),
        CanonicalContent::Thinking { .. } => None,
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
        // Gemini inlineData has no URL variant; pass the URL as text so the
        // model still sees the reference rather than silently dropping it.
        ImageSource::Url { url, .. } => GeminiPart::Text { text: url.clone() },
    }
}

fn tool_result_part(tool_use_id: &str, content: &[CanonicalContent], is_error: bool) -> GeminiPart {
    let text = flatten_text(content);
    let response = if is_error {
        json!({ "error": text })
    } else {
        json!({ "result": text })
    };
    GeminiPart::FunctionResponse {
        function_response: GeminiFunctionResponse {
            // Gemini keys responses by function name; the canonical tool_use_id
            // is the only stable handle available, so it doubles as the name.
            name: tool_use_id.to_owned(),
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
