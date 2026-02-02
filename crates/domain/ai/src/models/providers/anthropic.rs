use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicModels {
    #[serde(rename = "claude_3_opus")]
    pub opus: ModelConfig,
    #[serde(rename = "claude_3_sonnet")]
    pub sonnet: ModelConfig,
    #[serde(rename = "claude_3_haiku")]
    pub haiku: ModelConfig,
}

pub use systemprompt_models::ModelConfig;

impl Default for AnthropicModels {
    fn default() -> Self {
        Self {
            opus: ModelConfig {
                id: "claude-3-opus-20240229".to_string(),
                max_tokens: 200_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.075,
            },
            sonnet: ModelConfig {
                id: "claude-3-5-sonnet-20241022".to_string(),
                max_tokens: 200_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.015,
            },
            haiku: ModelConfig {
                id: "claude-3-5-haiku-20241022".to_string(),
                max_tokens: 200_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.004,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<AnthropicToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinking>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "tool")]
    Tool { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicThinking {
    #[serde(rename = "type")]
    pub thinking_type: String,
    pub budget_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: AnthropicImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicImageSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    pub r#type: String,
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AnthropicUsage {
    #[serde(rename = "input_tokens")]
    pub input: u32,
    #[serde(rename = "output_tokens")]
    pub output: u32,
    #[serde(default, rename = "cache_creation_input_tokens")]
    pub cache_creation: Option<u32>,
    #[serde(default, rename = "cache_read_input_tokens")]
    pub cache_read: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicMessageInfo },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlockInfo,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: AnthropicDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: AnthropicMessageDeltaInfo,
        usage: AnthropicDeltaUsage,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: AnthropicStreamError },
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicMessageInfo {
    pub id: String,
    pub model: String,
    pub role: String,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlockInfo {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicMessageDeltaInfo {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AnthropicDeltaUsage {
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicStreamError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicServerTool {
    #[serde(rename = "web_search_20250305")]
    WebSearch {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_uses: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicSearchRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub tools: Vec<AnthropicServerTool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicSearchResponse {
    pub id: String,
    pub content: Vec<AnthropicSearchContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: AnthropicSearchUsage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicSearchContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(default)]
        citations: Option<Vec<AnthropicCitation>>,
    },
    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult {
        tool_use_id: String,
        content: Vec<AnthropicWebSearchResultItem>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicCitation {
    pub r#type: String,
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub cited_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicWebSearchResultItem {
    #[serde(rename = "web_search_result")]
    WebSearchResult {
        url: String,
        title: String,
        #[serde(default)]
        page_age: Option<String>,
    },
    #[serde(rename = "web_search_tool_result_error")]
    Error {
        error_code: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AnthropicSearchUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub server_tool_use: Option<AnthropicServerToolUsage>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AnthropicServerToolUsage {
    #[serde(default)]
    pub web_search_requests: u32,
}
