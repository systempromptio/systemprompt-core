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
mod safety;
mod usage;

pub use request::{
    CacheControl, CacheTtl, CanonicalContent, CanonicalMessage, CanonicalReasoning,
    CanonicalRequest, CanonicalSampling, CanonicalTool, CanonicalToolChoice, CanonicalTooling,
    INTERRUPTED_TOOL_RESULT, ImageDetail, ImageSource, ReasoningEffort, ResponseFormat, Role,
    SearchConfig, SystemBlock, ThinkingConfig, TurnMerge, is_new_user_turn, normalize_turns,
};
pub use response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CodeExecutionOutput, ContentBlockKind,
    GroundedSource, Grounding,
};
pub use usage::{CanonicalUsage, CanonicalUsageUpdate};
