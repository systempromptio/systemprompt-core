use serde::{Deserialize, Serialize};

use super::AnthropicContentBlock;

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
