use serde::{Deserialize, Serialize};

use super::AnthropicMessage;

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
    Error { error_code: String },
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
