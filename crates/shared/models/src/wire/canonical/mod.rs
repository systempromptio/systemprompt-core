//! Provider-neutral request, response, and streaming-event model.
//!
//! Inbound adapters parse a wire request into a [`CanonicalRequest`] of
//! [`CanonicalMessage`]s carrying [`CanonicalContent`] parts; outbound adapters
//! render it back out and translate the upstream reply into a
//! [`CanonicalResponse`] or a stream of [`CanonicalEvent`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod response;

pub use request::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageDetail, ImageSource, ReasoningEffort, ResponseFormat, Role, SearchConfig, ThinkingConfig,
};
pub use response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, CodeExecutionOutput,
    ContentBlockKind, GroundedSource, Grounding,
};
