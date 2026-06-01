// JSON: protocol boundary — parse shape is owned by the models::wire Anthropic
// codec.
use serde_json::Value;
use systemprompt_models::wire::anthropic;

use super::super::super::canonical_response::CanonicalResponse;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    anthropic::parse_response(value, fallback_model)
}
