//! Tool-aware generation pipeline — executes tool calls, formats results
//! for the model, and synthesises the final response. See
//! [`crate::ToolResultFormatter`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod executor;
pub mod formatter;
pub mod synthesizer;

pub use executor::{ResponseStrategy, TooledExecutor};
pub use formatter::ToolResultFormatter;
pub use synthesizer::{
    FallbackGenerator, FallbackReason, ResponseSynthesizer, SynthesisParams,
    SynthesisPromptBuilder, SynthesisResult,
};
