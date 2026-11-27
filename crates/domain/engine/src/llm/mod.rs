//! Streaming model clients for the two wire protocols a systemprompt gateway
//! serves: Anthropic Messages (`POST {base}/messages`) and `OpenAI` Chat
//! Completions (`POST {base}/chat/completions`), both over SSE.
//!
//! The caller supplies everything that identifies or authorises a call: the
//! [`Endpoint`] (base URL, headers, body metadata) and the [`Transport`] that
//! sends the bytes. This crate shapes the request body from the neutral
//! [`Request`], parses the event stream and hands back unified [`Event`]s. It
//! never invents credentials, never retries on its own (see [`retry`] for the
//! pure policy) and never logs request bodies or header values.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod chat;
mod driver;
mod endpoint;
mod error;
mod event;
mod messages;
mod model;
mod overflow;
mod prepare;
mod repair;
mod response;
/// The retry policy as pure functions; the caller owns the loop that waits and
/// resends.
pub mod retry;
mod sse;
mod stream;
/// Rough token estimates for compaction budgeting.
pub mod tokens;
mod transport;

pub use endpoint::Endpoint;
pub use error::{LlmError, TransportError};
pub use event::{Event, FinishReason, Usage};
pub use model::{
    Attachment, Content, Message, ModelSpec, Params, Protocol, Reasoning, ReasoningEffort, Request,
    Role, Supports, ToolChoice, ToolDef,
};
pub use overflow::is_context_overflow;
pub use prepare::{prepare, request_body};
pub use repair::{ToolInput, repair_tool_input};
pub use response::{ModelResponse, ResponseHead};
pub use sse::{SseFrame, SseParser};
pub use stream::{EventStream, stream};
pub use transport::{ByteStream, HttpRequest, HttpResponse, HttpTransport, Timeouts, Transport};
