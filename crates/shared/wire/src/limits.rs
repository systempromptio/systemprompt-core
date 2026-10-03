//! Token limits of one upstream model.
//!
//! [`ModelLimits`] is the per-model ceiling a codec clamps a request to: the
//! context window, the output-token cap and the optional thinking budget. The
//! provider catalog carries one per model; the codecs read it when they render
//! an upstream request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelLimits {
    #[serde(default)]
    pub context_window: u32,

    #[serde(default)]
    pub max_output_tokens: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_thinking_budget: Option<u32>,
}
