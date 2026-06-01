// JSON: protocol boundary — body shape is owned by the models::wire Anthropic
// codec.
use serde_json::Value;
use systemprompt_models::wire::anthropic;

use super::super::super::canonical::CanonicalRequest;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_request_body(request: &CanonicalRequest, upstream_model: &str) -> Value {
    anthropic::build_request_body(request, upstream_model)
}
