//! Provider-agnostic response assembly shared across LLM backends.
//!
//! Re-exports [`build_response`] together with its [`BuildResponseParams`]
//! input and the [`TokenUsage`] accounting type, letting each provider adapter
//! turn a raw completion into a normalised `AiResponse` without duplicating the
//! logic.

mod response_builder;

pub use response_builder::{BuildResponseParams, TokenUsage, build_response};
