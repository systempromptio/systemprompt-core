//! Tool-aware generation pipeline — executes tool calls, formats results
//! for the model, and synthesises the final response. Result formatting is
//! [`systemprompt_models::ToolResultFormatter`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod executor;
pub mod synthesizer;

pub use executor::{ResponseStrategy, TooledExecutor};
pub use synthesizer::{
    FallbackGenerator, FallbackReason, ResponseSynthesizer, SynthesisParams,
    SynthesisPromptBuilder, SynthesisResult,
};
