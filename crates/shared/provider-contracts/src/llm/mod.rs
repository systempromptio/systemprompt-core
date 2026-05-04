//! Large-language-model provider contracts.
//!
//! Defines the [`LlmProvider`] trait used by domain crates to talk to
//! Anthropic, `OpenAI`, Gemini, or local models without binding to a
//! specific SDK, plus the request/response/streaming types that flow
//! across that boundary.
//!
//! ```no_run
//! use systemprompt_provider_contracts::llm::{ChatMessage, ChatRequest};
//!
//! let request = ChatRequest::new(vec![ChatMessage::user("Hello")], "claude-sonnet-4-7", 1024);
//! ```

mod error;
mod message;
mod provider;
mod request;
mod response;

pub use error::{LlmProviderError, LlmProviderResult};
pub use message::{ChatMessage, ChatRole};
pub use provider::{ChatStream, LlmProvider, ToolExecutionContext, ToolExecutor};
pub use request::{ChatRequest, SamplingParameters};
pub use response::{ChatResponse, TokenUsage};
