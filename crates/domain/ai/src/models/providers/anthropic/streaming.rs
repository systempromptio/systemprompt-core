use serde::Deserialize;

use super::AnthropicUsage;

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
    // JSON: external vendor identifier
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
