//! Gemini `generateContent` / `streamGenerateContent` wire codec.
//!
//! Builds a Google generativeLanguage v1beta request from a
//! [`crate::canonical::CanonicalRequest`], parses the buffered reply into
//! a [`crate::canonical::CanonicalResponse`], and maps the SSE byte
//! stream (`?alt=sse`) to [`crate::canonical::CanonicalEvent`]s.
//!
//! Gemini authenticates with an `x-goog-api-key` header (the `?key=` query
//! param is the alternative; this codec uses the header so keys stay out of
//! request lines and logs). The serde shapes of the Gemini bodies
//! (`GeminiRequest`, `GeminiResponse`, …) are public so other Gemini callers —
//! the image-generation provider — build on the same definitions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod request_parts;
mod response;
mod streaming;
mod streaming_parts;
mod thinking;
mod wire;

pub use request::build_request_body;
pub use response::{buffered_defect, parse_response, stop_reason};
pub use streaming::sse_to_canonical_events;
pub use wire::{
    GeminiCandidate, GeminiCodeExecutionResult, GeminiContent, GeminiEmpty, GeminiExecutableCode,
    GeminiFunctionCall, GeminiFunctionCallingConfig, GeminiFunctionDeclaration,
    GeminiFunctionResponse, GeminiGenerationConfig, GeminiGroundingChunk, GeminiGroundingMetadata,
    GeminiImageConfig, GeminiInlineData, GeminiPart, GeminiPromptFeedback, GeminiRequest,
    GeminiResponse, GeminiSystemInstruction, GeminiThinkingConfig, GeminiTool, GeminiToolConfig,
    GeminiUsageMetadata, GeminiWebSource,
};

pub const API_KEY_HEADER: &str = "x-goog-api-key";

#[must_use]
pub fn upstream_path(model: &str, stream: bool) -> String {
    if stream {
        format!("/models/{model}:streamGenerateContent?alt=sse")
    } else {
        format!("/models/{model}:generateContent")
    }
}
