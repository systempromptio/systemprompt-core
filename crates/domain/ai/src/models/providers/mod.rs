//! Wire-format request and response types for each upstream AI provider.
//!
//! One submodule per provider ([`anthropic`], [`gemini`], [`openai`]), each
//! mirroring that vendor's JSON API and exposing a `*Models` catalogue of the
//! supported model identifiers and their pricing defaults.

pub mod anthropic;
pub mod gemini;
pub mod openai;

pub use anthropic::AnthropicModels;
pub use gemini::GeminiModels;
pub use openai::OpenAiModels;
