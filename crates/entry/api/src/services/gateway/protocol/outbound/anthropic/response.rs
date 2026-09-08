//! Parses Anthropic responses into the canonical response.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — parse shape is owned by the models::wire Anthropic
// codec.
use serde_json::Value;
use systemprompt_models::wire::anthropic;
use systemprompt_models::wire::error::WireParseError;

use super::super::super::canonical_response::CanonicalResponse;

pub fn parse_response(
    value: &Value,
    fallback_model: &str,
) -> Result<CanonicalResponse, WireParseError> {
    anthropic::parse_response(value, fallback_model)
}
