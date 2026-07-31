//! Builds outbound Anthropic requests from the canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — body shape is owned by the models::wire Anthropic
// codec.
use bytes::Bytes;
use serde_json::{Map, Value};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::anthropic;

use super::super::super::canonical::CanonicalRequest;
use super::super::OutboundCtx;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_request_body(
    request: &CanonicalRequest,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) -> Value {
    anthropic::build_request_body(request, upstream_model, limits)
}

// Why: the passthrough lane must not become a way around the checks the
// canonical lane applies, so the policy transforms are re-applied here in
// place.
pub(super) fn normalize_raw_body(raw: &Bytes, ctx: &OutboundCtx<'_>) -> Option<Bytes> {
    let Ok(Value::Object(mut obj)) = serde_json::from_slice::<Value>(raw) else {
        return None;
    };
    obj.insert(
        "model".to_owned(),
        Value::String(ctx.upstream_model.to_owned()),
    );
    clamp_max_tokens(&mut obj, ctx.model_limits);
    anthropic::strip_user_id(&mut obj);
    match serde_json::to_vec(&Value::Object(obj)) {
        Ok(bytes) => Some(Bytes::from(bytes)),
        Err(e) => {
            tracing::warn!(error = %e, "re-encoding the passthrough body failed — rebuilding from canonical");
            None
        },
    }
}

fn clamp_max_tokens(obj: &mut Map<String, Value>, limits: Option<ModelLimits>) {
    let Some(requested) = obj.get("max_tokens").and_then(Value::as_u64) else {
        return;
    };
    let requested = u32::try_from(requested).unwrap_or(u32::MAX);
    let clamped = systemprompt_models::wire::clamp_output_tokens(
        requested,
        limits.map(|l| l.max_output_tokens),
    );
    if clamped != requested {
        obj.insert("max_tokens".to_owned(), Value::from(clamped));
    }
}
