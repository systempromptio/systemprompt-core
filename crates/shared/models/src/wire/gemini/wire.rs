//! Serde shapes for the Gemini v1beta `generateContent` wire format.
//!
//! These mirror the Google generativeLanguage request/response bodies. They are
//! defined locally so the wire codec does not depend on the agent-side provider
//! crate; the canonical codec is the single conversion point between them and
//! the provider-neutral model.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiRequest {
    pub(crate) contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_config: Option<GeminiToolConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GeminiSystemInstruction {
    pub(crate) parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiContent {
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) parts: Vec<GeminiPart>,
}

// Untagged so a single Vec<GeminiPart> can hold text, inline images, function
// calls (assistant tool use), and function responses (tool results).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum GeminiPart {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiInlineData {
    pub(crate) mime_type: String,
    pub(crate) data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiFunctionCall {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GeminiFunctionResponse {
    pub(crate) name: String,
    pub(crate) response: Value,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    pub(crate) function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GeminiFunctionDeclaration {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    pub(crate) parameters: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiToolConfig {
    pub(crate) function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiFunctionCallingConfig {
    pub(crate) mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allowed_function_names: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiResponse {
    #[serde(default)]
    pub(crate) candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    pub(crate) usage_metadata: Option<GeminiUsageMetadata>,
    #[serde(default)]
    pub(crate) response_id: Option<String>,
    #[serde(default)]
    pub(crate) model_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiCandidate {
    #[serde(default)]
    pub(crate) content: Option<GeminiContent>,
    #[serde(default)]
    pub(crate) finish_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiUsageMetadata {
    #[serde(default, rename = "promptTokenCount")]
    pub(crate) prompt: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    pub(crate) candidates: u32,
}
