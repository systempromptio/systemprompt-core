//! Builds outbound Anthropic requests from the canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — body shape is owned by the models::wire Anthropic
// codec.
use bytes::Bytes;
use serde_json::{Map, Value};
use systemprompt_wire::anthropic::BetaHeader;
use systemprompt_wire::{ModelLimits, WireProtocol, anthropic};

use super::super::super::canonical::CanonicalRequest;
use super::super::OutboundCtx;

// JSON: Anthropic Messages upstream body — passthrough keeps unknown client
// fields.
pub fn build_request_body(
    request: &CanonicalRequest,
    upstream_model: &str,
    limits: Option<ModelLimits>,
) -> Value {
    anthropic::build_request_body(request, upstream_model, limits)
}

pub(super) fn dropped_betas(ctx: &OutboundCtx<'_>) -> BetaHeader {
    let mut dropped = ctx
        .upstream
        .dropped_betas(WireProtocol::Anthropic, ctx.forward_headers);
    let learned = super::rejected_betas::learned(ctx.route.provider.as_str());
    dropped.extend(BetaHeader::parse(
        &learned.iter().cloned().collect::<Vec<_>>().join(","),
    ));
    dropped
}

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
    let provider = ctx.route.provider.as_str();
    drop_refused(
        &mut obj,
        provider,
        &dropped_betas(ctx),
        &super::refused_fields::learned(provider),
    );
    if ctx.automatic_prompt_caching && !ctx.request.has_cache_control() {
        obj.insert(
            "cache_control".to_owned(),
            anthropic::cache_control_to_anthropic(
                super::super::super::canonical::CacheControl::EPHEMERAL,
            ),
        );
    }
    ctx.upstream.finish_body(WireProtocol::Anthropic, &mut obj);
    match serde_json::to_vec(&Value::Object(obj)) {
        Ok(bytes) => Some(Bytes::from(bytes)),
        Err(e) => {
            tracing::warn!(error = %e, "re-encoding the passthrough body failed — rebuilding from canonical");
            None
        },
    }
}

// JSON: Anthropic Messages upstream body — passthrough keeps unknown client
// fields.
pub(super) fn enable_automatic_prompt_caching(body: &mut Value, ctx: &OutboundCtx<'_>) {
    if !ctx.automatic_prompt_caching || ctx.request.has_cache_control() {
        return;
    }
    let Some(obj) = body.as_object_mut() else {
        return;
    };
    obj.insert(
        "cache_control".to_owned(),
        anthropic::cache_control_to_anthropic(
            super::super::super::canonical::CacheControl::EPHEMERAL,
        ),
    );
}

// JSON: Anthropic Messages upstream body — passthrough keeps unknown client
// fields.
fn clamp_max_tokens(obj: &mut Map<String, Value>, limits: Option<ModelLimits>) {
    let Some(requested) = obj.get("max_tokens").and_then(Value::as_u64) else {
        return;
    };
    let requested = u32::try_from(requested).unwrap_or(u32::MAX);
    let clamped =
        systemprompt_wire::clamp_output_tokens(requested, limits.map(|l| l.max_output_tokens));
    if clamped != requested {
        obj.insert("max_tokens".to_owned(), Value::from(clamped));
    }
}

// JSON: Anthropic Messages upstream body — passthrough keeps unknown client
// fields.
fn drop_refused(
    obj: &mut Map<String, Value>,
    provider: &str,
    dropped_betas: &BetaHeader,
    refused_fields: &std::collections::BTreeSet<String>,
) {
    let mut removed: Vec<String> = anthropic::strip_fields_gated_by(obj, dropped_betas)
        .into_iter()
        .map(str::to_owned)
        .collect();
    removed.extend(
        refused_fields
            .iter()
            .filter(|field| obj.remove(field.as_str()).is_some())
            .cloned(),
    );
    if !removed.is_empty() {
        tracing::info!(
            provider,
            fields = ?removed,
            dropped_betas = ?dropped_betas.render(),
            "body fields the upstream is not sent"
        );
    }
}

pub(super) fn carries_any_field(body: &Bytes, fields: &std::collections::BTreeSet<String>) -> bool {
    if fields.is_empty() {
        return false;
    }
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| {
            v.as_object()
                .map(|o| fields.iter().any(|f| o.contains_key(f)))
        })
        .unwrap_or(false)
}

pub(super) fn without_refused(
    body: &Bytes,
    provider: &str,
    dropped_betas: &BetaHeader,
    fields: &std::collections::BTreeSet<String>,
) -> Bytes {
    let Ok(Value::Object(mut obj)) = serde_json::from_slice::<Value>(body) else {
        return body.clone();
    };
    drop_refused(&mut obj, provider, dropped_betas, fields);
    serde_json::to_vec(&Value::Object(obj)).map_or_else(|_| body.clone(), Bytes::from)
}
