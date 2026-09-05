//! The OpenAI-chat passthrough lane.
//!
//! The caller's body is forwarded byte-preserving so unknown provider
//! parameters survive, with only the gateway-owned fields rewritten. Each of
//! those rewrites is a control the caller must not be able to defeat, and
//! every one of them fails silently: the request succeeds either way, and the
//! damage — a leaked identifier, an unbilled stream, an unclamped limit —
//! shows up somewhere else entirely.

use bytes::Bytes;
use serde_json::{Value, json};
use std::collections::HashMap;
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::OutboundCtx;
use systemprompt_api::services::gateway::protocol::outbound::openai_chat::test_api::normalize_raw_body;
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::GatewayRoute;
use systemprompt_models::services::ai::ModelLimits;

fn route() -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("r1"),
        model_pattern: "*".into(),
        provider: ProviderId::new("openai"),
        upstream_model: Some("upstream-1".into()),
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
    }
}

fn canonical() -> CanonicalRequest {
    CanonicalRequest {
        model: "caller-model".into(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hi".into())],
        }],
        max_tokens: 64,
        ..Default::default()
    }
}

fn normalize(body: &Value, limits: Option<ModelLimits>) -> Value {
    let raw = Bytes::from(serde_json::to_vec(body).expect("encode body"));
    let route = route();
    let request = canonical();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: "https://upstream.invalid/v1/chat/completions",
        api_key: "sk-test",
        api_key_is_bearer: false,
        request: &request,
        upstream_model: "upstream-1",
        model_limits: limits,
        forward_headers: &[],
        raw_body: Some(&raw),
    };

    let out = normalize_raw_body(&raw, &ctx).expect("a well-formed body normalises");
    serde_json::from_slice(&out).expect("the normalised body is JSON")
}

// Why: `user` carries the caller's own end-user identifier. The gateway strips
// it so the developer's identifiers never reach the upstream provider — a leak
// that no error surfaces and no response reveals.
#[test]
fn the_caller_identity_field_never_reaches_the_upstream() {
    let out = normalize(
        &json!({"model": "caller-model", "user": "end-user-42", "messages": []}),
        None,
    );

    assert!(
        out.get("user").is_none(),
        "the caller-identity field must be stripped: {out}"
    );
}

// Why: the model is the route's decision, not the caller's. Forwarding the
// caller's choice would let any client reach any model the credential can
// serve, bypassing routing entirely.
#[test]
fn the_model_is_replaced_with_the_routes_upstream_model() {
    let out = normalize(
        &json!({"model": "something-expensive", "messages": []}),
        None,
    );

    assert_eq!(out["model"], "upstream-1");
}

// Why: unknown provider parameters are the point of the passthrough lane —
// they survive so a caller can use provider features the canonical form does
// not model.
#[test]
fn unrecognised_caller_parameters_survive_the_rewrite() {
    let out = normalize(
        &json!({"model": "m", "messages": [], "logit_bias": {"50256": -100}, "seed": 7}),
        None,
    );

    assert_eq!(out["seed"], 7);
    assert_eq!(out["logit_bias"]["50256"], -100);
}

// Why: the clamp is what stops a caller asking for more output than the model
// card allows. Unclamped, the upstream either rejects the request or bills for
// a ceiling the operator never agreed to.
#[test]
fn an_output_limit_above_the_model_card_is_clamped_down() {
    let limits = ModelLimits {
        context_window: 100_000,
        max_output_tokens: 1_000,
        ..Default::default()
    };

    let out = normalize(
        &json!({"model": "m", "messages": [], "max_completion_tokens": 999_999}),
        Some(limits),
    );

    assert_eq!(out["max_completion_tokens"], 1_000);
}

#[test]
fn an_output_limit_within_the_model_card_is_left_alone() {
    let limits = ModelLimits {
        context_window: 100_000,
        max_output_tokens: 1_000,
        ..Default::default()
    };

    let out = normalize(
        &json!({"model": "m", "messages": [], "max_completion_tokens": 500}),
        Some(limits),
    );

    assert_eq!(out["max_completion_tokens"], 500);
}

// Why: the older spelling is still what many clients send. Clamping only the
// new one leaves the legacy field an unclamped way past the model card.
#[test]
fn the_legacy_max_tokens_spelling_is_clamped_too() {
    let limits = ModelLimits {
        context_window: 100_000,
        max_output_tokens: 1_000,
        ..Default::default()
    };

    let out = normalize(
        &json!({"model": "m", "messages": [], "max_tokens": 999_999}),
        Some(limits),
    );

    assert_eq!(out["max_tokens"], 1_000);
}

// Why: usage reporting is what feeds the cost pipeline. A streamed request
// that does not report usage is served and never billed.
#[test]
fn a_streamed_request_is_forced_to_report_usage() {
    let out = normalize(&json!({"model": "m", "messages": [], "stream": true}), None);

    assert_eq!(
        out["stream_options"]["include_usage"], true,
        "a stream that does not report usage is never billed: {out}"
    );
}

// Why: the caller may already send `stream_options` for other reasons. Those
// must survive while `include_usage` is still forced on.
#[test]
fn forcing_usage_preserves_the_callers_other_stream_options() {
    let out = normalize(
        &json!({
            "model": "m",
            "messages": [],
            "stream": true,
            "stream_options": {"include_usage": false, "something_else": "kept"}
        }),
        None,
    );

    assert_eq!(
        out["stream_options"]["include_usage"], true,
        "the caller must not be able to opt out of usage reporting"
    );
    assert_eq!(out["stream_options"]["something_else"], "kept");
}

#[test]
fn a_non_streamed_request_is_not_given_stream_options() {
    let out = normalize(
        &json!({"model": "m", "messages": [], "stream": false}),
        None,
    );

    assert!(
        out.get("stream_options").is_none(),
        "a buffered request has no usage stream to configure: {out}"
    );
}

// Why: the lane only applies to a JSON object body. Anything else must fall
// through to the canonical rebuild rather than being forwarded unrewritten,
// which would skip every control above.
#[test]
fn a_body_that_is_not_a_json_object_declines_the_passthrough() {
    let raw = Bytes::from_static(b"not json at all");
    let route = route();
    let request = canonical();
    let ctx = OutboundCtx {
        route: &route,
        endpoint: "https://upstream.invalid/v1/chat/completions",
        api_key: "sk-test",
        api_key_is_bearer: false,
        request: &request,
        upstream_model: "upstream-1",
        model_limits: None,
        forward_headers: &[],
        raw_body: Some(&raw),
    };

    assert!(
        normalize_raw_body(&raw, &ctx).is_none(),
        "an unparseable body must not be forwarded through the passthrough lane"
    );
}
