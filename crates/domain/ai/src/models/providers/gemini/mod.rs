mod request;
mod response;

pub use request::*;
pub use response::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModels {
    pub gemini_flash_lite: ModelConfig,
    pub gemini_flash: ModelConfig,
}

pub use systemprompt_models::ModelConfig;

impl Default for GeminiModels {
    fn default() -> Self {
        Self {
            gemini_flash_lite: ModelConfig {
                id: "gemini-2.5-flash-lite".to_string(),
                max_tokens: 1_000_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.0004,
            },
            gemini_flash: ModelConfig {
                id: "gemini-2.5-flash".to_string(),
                max_tokens: 1_000_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.0025,
            },
        }
    }
}
