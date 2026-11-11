//! Gemini `usageMetadata`: token counts read as `u64` and saturated to `u32`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

use crate::canonical::deserialize_wire_count as count;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUsageMetadata {
    #[serde(default, rename = "promptTokenCount", deserialize_with = "count")]
    pub prompt: u32,
    #[serde(default, rename = "candidatesTokenCount", deserialize_with = "count")]
    pub candidates: u32,
    #[serde(default, rename = "totalTokenCount", deserialize_with = "count")]
    pub total: u32,
    #[serde(
        default,
        rename = "cachedContentTokenCount",
        deserialize_with = "count"
    )]
    pub cached: u32,
    // Why: Gemini counts `thoughtsTokenCount` in `totalTokenCount`, outside
    // `candidatesTokenCount`.
    #[serde(default, rename = "thoughtsTokenCount", deserialize_with = "count")]
    pub thoughts: u32,
}
