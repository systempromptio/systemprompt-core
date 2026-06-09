//! Renders a [`CanonicalRequest`] into a Gemini `generateContent` body.

use serde_json::{Value, json};

use super::wire::{
    GeminiContent, GeminiEmpty, GeminiFunctionCall, GeminiFunctionCallingConfig,
    GeminiFunctionDeclaration, GeminiFunctionResponse, GeminiGenerationConfig, GeminiInlineData,
    GeminiPart, GeminiRequest, GeminiSystemInstruction, GeminiThinkingConfig, GeminiTool,
    GeminiToolConfig,
};
use crate::profile::WireProtocol;
use crate::schema::SchemaSanitizer;
use crate::services::ai::ModelLimits;
use crate::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalToolChoice, ImageSource,
    ResponseFormat, Role,
};

#[must_use]
pub fn build_request_body(request: &CanonicalRequest, limits: Option<ModelLimits>) -> Value {
    let body = GeminiRequest {
        contents: contents(request),
        system_instruction: request.system.as_ref().map(|s| GeminiSystemInstruction {
            parts: vec![GeminiPart::Text { text: s.clone() }],
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
        max_output_tokens: Some(crate::wire::clamp_output_tokens(
            request.max_tokens,
            limits.map(|l| l.max_output_tokens),
        )),
        stop_sequences: if request.stop_sequences.is_empty() {
            None
        } else {
            Some(request.stop_sequences.clone())
        },
        response_mime_type,
        response_schema,
        thinking_config: thinking_config(request, limits.and_then(|l| l.max_thinking_budget)),
    }
}

fn thinking_config(
    request: &CanonicalRequest,
    max_thinking_budget: Option<u32>,
) -> Option<GeminiThinkingConfig> {
    let thinking = request.thinking?;
    if !thinking.enabled {
        return None;
    }
    let thinking_budget = match (thinking.budget_tokens, max_thinking_budget) {
        (Some(requested), Some(cap)) => Some(requested.min(cap)),
        (requested, _) => requested,
    };
    Some(GeminiThinkingConfig {
        thinking_budget,
        include_thoughts: None,
    })
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
        CanonicalContent::ToolUse {
            name,
            input,
            signature,
            ..
        } => Some(GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: name.clone(),
                args: input.clone(),
            },
            thought_signature: signature.clone(),
        }),
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured_content,
            ..
        } => Some(tool_result_part(
            tool_use_id,
            content,
            *is_error,
            structured_content.as_ref(),
        )),
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
        ImageSource::Url { url, .. } => GeminiPart::Text { text: url.clone() },
    }
}

fn tool_result_part(
    tool_use_id: &str,
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
