use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiModels {
    pub gpt4_turbo: ModelConfig,
    pub gpt35_turbo: ModelConfig,
}

pub use systemprompt_models::ModelConfig;

impl Default for OpenAiModels {
    fn default() -> Self {
        Self {
            gpt4_turbo: ModelConfig {
                id: "gpt-4-turbo".to_string(),
                max_tokens: 128_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.03,
            },
            gpt35_turbo: ModelConfig {
                id: "gpt-3.5-turbo".to_string(),
                max_tokens: 16385,
                supports_tools: true,
                cost_per_1k_tokens: 0.0015,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OpenAiResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<OpenAiReasoningEffort>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAiReasoningEffort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: OpenAiMessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenAiMessageContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenAiContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenAiImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiTool {
    pub r#type: String,
    pub function: OpenAiFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAiChoice>,
    pub usage: Option<OpenAiUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiChoice {
    pub index: i32,
    pub message: OpenAiResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiToolCall {
    pub id: String,
    pub r#type: String,
    pub function: OpenAiFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OpenAiPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub prompt_tokens_details: Option<OpenAiPromptTokensDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenAiResponseFormat {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "json_object")]
    JsonObject,
    #[serde(rename = "json_schema")]
    JsonSchema { json_schema: OpenAiJsonSchema },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiJsonSchema {
    pub name: String,
    pub schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiResponsesRequest {
    pub model: String,
    pub input: Vec<OpenAiResponsesInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAiResponsesTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiResponsesInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenAiResponsesTool {
    #[serde(rename = "web_search")]
    WebSearch {
        #[serde(skip_serializing_if = "Option::is_none")]
        search_context_size: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiResponsesResponse {
    pub id: String,
    pub output: Vec<OpenAiResponsesOutput>,
    #[serde(default)]
    pub usage: Option<OpenAiResponsesUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiResponsesOutput {
    pub r#type: String,
    #[serde(default)]
    pub content: Option<Vec<OpenAiResponsesContent>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiResponsesContent {
    pub r#type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub annotations: Option<Vec<OpenAiWebSearchAnnotation>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiWebSearchAnnotation {
    pub r#type: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub start_index: Option<u32>,
    #[serde(default)]
    pub end_index: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct OpenAiResponsesUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}
