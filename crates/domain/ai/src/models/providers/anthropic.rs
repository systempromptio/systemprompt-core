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
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub stop_sequences: Option<Vec<String>>,
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<AnthropicToolChoice>,
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
