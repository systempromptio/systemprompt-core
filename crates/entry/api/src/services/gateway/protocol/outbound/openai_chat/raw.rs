//! Passthrough lane for OpenAI-chat-to-OpenAI-chat relays.
//!
//! When the inbound wire is already Chat Completions, the caller's body is
//! forwarded byte-preserving (unknown provider parameters survive) with only
//! the gateway-owned fields rewritten: the model is replaced with the route's
//! upstream model, the output-token limit is clamped to the model card,
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

pub(super) fn normalize_raw_body(raw: &Bytes, ctx: &OutboundCtx<'_>) -> Option<Bytes> {
    let Ok(Value::Object(mut obj)) = serde_json::from_slice::<Value>(raw) else {
        return None;
    };
    obj.insert(
        "model".to_owned(),
        Value::String(ctx.upstream_model.to_owned()),
    );
    clamp_output_limit(&mut obj, ctx.model_limits);
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

fn clamp_output_limit(obj: &mut Map<String, Value>, limits: Option<ModelLimits>) {
    for field in ["max_completion_tokens", "max_tokens"] {
        let Some(requested) = obj.get(field).and_then(Value::as_u64) else {
            continue;
        };
        let requested = u32::try_from(requested).unwrap_or(u32::MAX);
        let clamped = systemprompt_models::wire::clamp_output_tokens(
            requested,
            limits.map(|l| l.max_output_tokens),
        );
        if clamped != requested {
            obj.insert(field.to_owned(), Value::from(clamped));
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
