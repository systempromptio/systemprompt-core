//! The sampling, tooling and reasoning groups of a canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::options::{
    CanonicalTool, CanonicalToolChoice, ReasoningEffort, SearchConfig, ThinkingConfig,
};

#[derive(Debug, Clone, Default)]
pub struct CanonicalSampling {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub stop_sequences: Vec<String>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct CanonicalTooling {
    pub tools: Vec<CanonicalTool>,
    pub tool_choice: Option<CanonicalToolChoice>,
    pub search: Option<SearchConfig>,
    pub code_execution: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalReasoning {
    pub thinking: Option<ThinkingConfig>,
    pub reasoning_effort: Option<ReasoningEffort>,
}
