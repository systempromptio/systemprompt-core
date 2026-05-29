//! Google Gemini `generateContent` API wire types.
//!
//! Re-exports the request and response structs matching Gemini's JSON shape,
//! plus [`GeminiModels`] — the default Flash/Flash-Lite catalogue with
//! per-model token limits and pricing.

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
                id: "gemini-3.1-flash-lite-preview".to_owned(),
                max_tokens: 1_000_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.00025,
            },
            gemini_flash: ModelConfig {
                id: "gemini-2.5-flash".to_owned(),
                max_tokens: 1_000_000,
                supports_tools: true,
                cost_per_1k_tokens: 0.0025,
            },
        }
    }
}
