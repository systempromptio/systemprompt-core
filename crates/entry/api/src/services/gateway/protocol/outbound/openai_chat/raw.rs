//! Passthrough lane for OpenAI-chat-to-OpenAI-chat relays.
//!
//! When the inbound wire is already Chat Completions, the caller's body is
//! forwarded byte-preserving (unknown provider parameters survive) with only
//! the gateway-owned fields rewritten: the model is replaced with the route's
//! upstream model, the output-token limit is sized by the model card (clamped
//! down to its cap, or set to that cap for a reasoning model, which bills its
//! thinking from the same completion budget),
//! streamed requests are forced to report usage (`stream_options.include_usage`
//! feeds the cost pipeline), and the caller-identity `user` field is stripped
//! so the developer never reaches the upstream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::{Map, Value, json};
use systemprompt_models::services::ai::ModelLimits;

use super::super::OutboundCtx;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported by the feature-gated `test_api` module"
    )
)]
pub fn normalize_raw_body(raw: &Bytes, ctx: &OutboundCtx<'_>) -> Option<Bytes> {
    let Ok(Value::Object(mut obj)) = serde_json::from_slice::<Value>(raw) else {
        return None;
    };
    obj.insert(
        "model".to_owned(),
        Value::String(ctx.upstream_model.to_owned()),
    );
    apply_output_limit(&mut obj, ctx.upstream_model, ctx.model_limits);
    if obj.get("stream").and_then(Value::as_bool) == Some(true) {
        force_include_usage(&mut obj);
    }
    obj.remove("user");
    match serde_json::to_vec(&Value::Object(obj)) {
        Ok(bytes) => Some(Bytes::from(bytes)),
        Err(e) => {
            tracing::warn!(error = %e, "re-encoding the passthrough body failed — rebuilding from canonical");
            None
        },
    }
}

// Why: both spellings, because a caller may send either and the upstream
// honours whichever it finds. A reasoning model gets the full model-card cap:
// it bills thought against the same completion budget, so the caller's limit
// -- which bounds visible output -- starves the turn and it stops on `length`
// before the tool call is ever emitted.
fn apply_output_limit(
    obj: &mut Map<String, Value>,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) {
    for field in ["max_completion_tokens", "max_tokens"] {
        let Some(requested) = obj.get(field).and_then(Value::as_u64) else {
            continue;
        };
        let requested = u32::try_from(requested).unwrap_or(u32::MAX);
        let allowed = systemprompt_models::wire::openai_chat::passthrough_output_tokens(
            requested,
            upstream_model,
            limits,
        );
        if allowed != requested {
            obj.insert(field.to_owned(), Value::from(allowed));
        }
    }
}

fn force_include_usage(obj: &mut Map<String, Value>) {
    match obj.get_mut("stream_options") {
        Some(Value::Object(opts)) => {
            opts.insert("include_usage".to_owned(), Value::Bool(true));
        },
        _ => {
            obj.insert(
                "stream_options".to_owned(),
                json!({ "include_usage": true }),
            );
        },
    }
}
