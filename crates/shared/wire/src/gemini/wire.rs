//! Serde shapes for the Gemini v1beta `generateContent` wire format.
//!
//! These mirror the Google generativeLanguage request/response bodies. They are
//! the one definition of the Gemini shapes: the canonical codec converts
//! between them and the provider-neutral model, and the image-generation
//! provider in `systemprompt-ai` builds its `generateContent` request from
//! them (`responseModalities`, `imageConfig`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<GeminiToolConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeminiSystemInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    pub role: String,
    #[serde(default)]
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeminiPart {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thought: Option<bool>,
        #[serde(
            rename = "thoughtSignature",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        thought_signature: Option<String>,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: GeminiInlineData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
        // Why: Gemini 3.x requires `thoughtSignature` replayed on the same part as
        // `functionCall`, not nested inside it.
        #[serde(
            rename = "thoughtSignature",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        thought_signature: Option<String>,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
    ExecutableCode {
        #[serde(rename = "executableCode")]
        executable_code: GeminiExecutableCode,
    },
    CodeExecutionResult {
        #[serde(rename = "codeExecutionResult")]
        code_execution_result: GeminiCodeExecutionResult,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiInlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCall {
    pub name: String,
    #[serde(default)]
    // JSON: Gemini `functionCall.args` / `functionResponse.response` are the tool's own JSON.
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiFunctionResponse {
    pub name: String,
    // JSON: Gemini `functionCall.args` / `functionResponse.response` are the tool's own JSON.
    pub response: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiExecutableCode {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCodeExecutionResult {
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    // JSON: Gemini `responseSchema` is a JSON Schema document.
    pub response_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<GeminiThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_modalities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_config: Option<GeminiImageConfig>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiImageConfig {
    pub aspect_ratio: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_size: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum GeminiTool {
    Functions {
        #[serde(rename = "functionDeclarations")]
        function_declarations: Vec<GeminiFunctionDeclaration>,
    },
    GoogleSearch {
        #[serde(rename = "googleSearch")]
        google_search: GeminiEmpty,
    },
    UrlContext {
        #[serde(rename = "urlContext")]
        url_context: GeminiEmpty,
    },
    CodeExecution {
        #[serde(rename = "codeExecution")]
        code_execution: GeminiEmpty,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "Gemini expects these tool markers as an empty JSON object `{}`; a unit struct would \
              serialize as `null`"
)]
pub struct GeminiEmpty {}

#[derive(Debug, Clone, Serialize)]
pub struct GeminiFunctionDeclaration {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    // JSON: Gemini `functionDeclaration.parameters` is a JSON Schema document.
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiToolConfig {
    pub function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCallingConfig {
    pub mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    #[serde(default)]
    pub candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    pub prompt_feedback: Option<GeminiPromptFeedback>,
    #[serde(default)]
    pub usage_metadata: Option<GeminiUsageMetadata>,
    #[serde(default)]
    pub response_id: Option<String>,
    #[serde(default)]
    pub model_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    #[serde(default)]
    pub content: Option<GeminiContent>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    // Why: Gemini explains a non-STOP finish here — the malformed call text,
    // the safety category — and nowhere else.
    #[serde(default)]
    pub finish_message: Option<String>,
    #[serde(default)]
    pub grounding_metadata: Option<GeminiGroundingMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPromptFeedback {
    #[serde(default)]
    pub block_reason: Option<String>,
    #[serde(default)]
    pub block_reason_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGroundingMetadata {
    #[serde(default)]
    pub grounding_chunks: Vec<GeminiGroundingChunk>,
    #[serde(default)]
    pub web_search_queries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeminiGroundingChunk {
    #[serde(default)]
    pub web: Option<GeminiWebSource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeminiWebSource {
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUsageMetadata {
    #[serde(default, rename = "promptTokenCount")]
    pub prompt: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    pub candidates: u32,
    #[serde(default, rename = "totalTokenCount")]
    pub total: u32,
    #[serde(default, rename = "cachedContentTokenCount")]
    pub cached: u32,
    // Why: Gemini counts `thoughtsTokenCount` in `totalTokenCount`, outside
    // `candidatesTokenCount`.
    #[serde(default, rename = "thoughtsTokenCount")]
    pub thoughts: u32,
}
