//! Provider-neutral request, response, and streaming-event model.
//!
//! Inbound adapters parse a wire request into a [`CanonicalRequest`] of
//! [`CanonicalMessage`]s carrying [`CanonicalContent`] parts; outbound adapters
//! render it back out and translate the upstream reply into a
//! [`CanonicalResponse`] or a stream of [`CanonicalEvent`]s.

mod request;
mod response;

pub use request::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, Role, ThinkingConfig,
};
pub use response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
