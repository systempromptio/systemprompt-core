mod request;
mod response;
mod search;
mod streaming;

use serde::{Deserialize, Serialize};

pub use request::{
    AnthropicContent, AnthropicContentBlock, AnthropicImageSource, AnthropicMessage,
    AnthropicRequest, AnthropicThinking, AnthropicTool, AnthropicToolChoice,
};
pub use response::{AnthropicResponse, AnthropicUsage};
pub use search::{
    AnthropicCitation, AnthropicSearchContentBlock, AnthropicSearchRequest,
    AnthropicSearchResponse, AnthropicSearchUsage, AnthropicServerTool, AnthropicServerToolUsage,
    AnthropicWebSearchResultItem,
};
pub use streaming::{
    AnthropicContentBlockInfo, AnthropicDelta, AnthropicDeltaUsage, AnthropicMessageDeltaInfo,
    AnthropicMessageInfo, AnthropicStreamError, AnthropicStreamEvent,
};
pub use systemprompt_models::ModelConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicModels {
    #[serde(rename = "claude_3_opus")]
    pub opus: ModelConfig,
    #[serde(rename = "claude_3_sonnet")]
    pub sonnet: ModelConfig,
    #[serde(rename = "claude_3_haiku")]
    pub haiku: ModelConfig,
}

impl Default for AnthropicModels {
    fn default() -> Self {
        Self {
            opus: ModelConfig {
                id: "claude-opus-4-6-20250610".to_owned(),
                max_tokens: 200_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.025,
            },
            sonnet: ModelConfig {
                id: "claude-sonnet-4-6-20250610".to_owned(),
                max_tokens: 200_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.015,
            },
            haiku: ModelConfig {
                id: "claude-haiku-4-5-20251101".to_owned(),
                max_tokens: 200_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.005,
            },
        }
    }
}
