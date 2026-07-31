//! Inspection-surface extraction over forwarded wire bodies.
//!
//! The gateway attaches this surface to the canonical request so scanners read
//! a superset of what is actually forwarded upstream. These tests pin the two
//! properties that makes true: it finds strings wherever they sit in the JSON,
//! and it stays bounded on a body an attacker controls.

use serde_json::json;
use systemprompt_models::wire::inspect::{SurfaceBudget, sse_string_leaves, string_leaves};

fn surface(value: &serde_json::Value) -> Vec<String> {
    string_leaves(
        &serde_json::to_vec(value).expect("serialize"),
        SurfaceBudget::default(),
    )
    .leaves()
    .iter()
    .map(|leaf| leaf.value.clone())
    .collect()
}

#[test]
fn finds_strings_nested_anywhere_including_shapes_the_canonical_model_drops() {
    // Why: this is the regression shape — a `document` block is dropped by the
    // inbound parser, so nothing canonical can see a credential placed here.
    let values = surface(&json!({
        "model": "claude-x",
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "innocuous" },
                { "type": "document", "source": { "data": "AKIAIOSFODNN7EXAMPLE" } },
                { "type": "tool_result", "structuredContent": { "token": "ghp_deadbeef" } }
            ]
        }],
        "context_management": { "note": "sk-ant-hidden" }
    }));

    for expected in [
        "AKIAIOSFODNN7EXAMPLE",
        "ghp_deadbeef",
        "sk-ant-hidden",
        "innocuous",
    ] {
        assert!(
            values.iter().any(|v| v == expected),
            "{expected} must appear in the inspection surface, got {values:?}"
        );
    }
}

#[test]
fn object_keys_are_leaves_too() {
    let values = surface(&json!({ "AKIAIOSFODNN7EXAMPLE": "v" }));

    assert!(
        values.iter().any(|v| v == "AKIAIOSFODNN7EXAMPLE"),
        "a credential used as a key must still be inspected, got {values:?}"
    );
}

#[test]
fn leaves_are_reported_in_document_order() {
    let values = surface(&json!({ "a": "first", "b": ["second", "third"] }));

    let positions: Vec<usize> = ["first", "second", "third"]
        .iter()
        .map(|needle| {
            values
                .iter()
                .position(|v| v == needle)
                .unwrap_or_else(|| panic!("{needle} missing from {values:?}"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "document order must be preserved, got {values:?}"
    );
}

/// Builds `{"n":{"n":...{"n":"<leaf>"}}}` nested `depth` deep, as text.
///
/// Written as a string rather than through `json!` because `serde_json::Value`
/// drops recursively: constructing a deep value would overflow inside the test
/// harness before the code under test ever ran.
fn nested_body(depth: usize, leaf: &str) -> Vec<u8> {
    let mut s = String::new();
    for _ in 0..depth {
        s.push_str("{\"n\":");
    }
    s.push('"');
    s.push_str(leaf);
    s.push('"');
    for _ in 0..depth {
        s.push('}');
    }
    s.into_bytes()
}

#[test]
fn nesting_past_the_depth_cap_truncates_instead_of_recursing() {
    let body = nested_body(100, "AKIAIOSFODNN7EXAMPLE");

    let surface = string_leaves(&body, SurfaceBudget::default());

    assert!(
        surface.truncated(),
        "a body past the depth cap must report truncation"
    );
    assert!(
        !surface.leaves().iter().any(|l| l.value.contains("AKIA")),
        "the leaf below the cap is not inspected, which is exactly why \
         truncation has to be reported rather than swallowed"
    );
}

#[test]
fn nesting_within_the_depth_cap_is_walked_fully() {
    let body = nested_body(32, "AKIAIOSFODNN7EXAMPLE");

    let surface = string_leaves(&body, SurfaceBudget::default());

    assert!(!surface.truncated());
    assert!(
        surface
            .leaves()
            .iter()
            .any(|l| l.value == "AKIAIOSFODNN7EXAMPLE"),
        "a leaf inside the cap must be inspected however deep it sits"
    );
}

#[test]
fn a_body_too_deep_for_the_json_parser_yields_an_empty_surface() {
    // Why: serde_json refuses this body outright, so the surface is empty. That
    // is safe only because the same parse failure stops the gateway taking the
    // passthrough lane — an unreadable body is rebuilt, never relayed unread.
    let surface = string_leaves(&nested_body(5_000, "x"), SurfaceBudget::default());

    assert!(surface.is_empty());
}

#[test]
fn exhausting_the_leaf_budget_reports_truncation() {
    let items: Vec<String> = (0..64).map(|i| format!("value-{i}")).collect();
    let budget = SurfaceBudget {
        leaves: 8,
        ..SurfaceBudget::default()
    };

    let surface = string_leaves(&serde_json::to_vec(&items).expect("serialize"), budget);

    assert_eq!(surface.len(), 8);
    assert!(
        surface.truncated(),
        "a partial surface must never be reported as complete"
    );
}

#[test]
fn an_oversized_leaf_keeps_its_head_and_its_tail() {
    // Why: a credential pasted into a large blob sits at one end far more often
    // than in the middle, and keeping both ends costs the same as keeping one.
    let value = format!("HEAD-MARKER{}TAIL-MARKER", "x".repeat(4_000));
    let budget = SurfaceBudget {
        leaf_bytes: 128,
        ..SurfaceBudget::default()
    };

    let surface = string_leaves(
        &serde_json::to_vec(&json!(value)).expect("serialize"),
        budget,
    );
    let kept = &surface.leaves()[0].value;

    assert!(kept.contains("HEAD-MARKER"), "{kept}");
    assert!(kept.contains("TAIL-MARKER"), "{kept}");
    assert!(surface.truncated());
}

#[test]
fn a_multibyte_leaf_clips_on_a_character_boundary() {
    let value = "é".repeat(4_000);
    let budget = SurfaceBudget {
        leaf_bytes: 101,
        ..SurfaceBudget::default()
    };

    let surface = string_leaves(
        &serde_json::to_vec(&json!(value)).expect("serialize"),
        budget,
    );

    assert_eq!(surface.len(), 1, "clipping must not drop the leaf");
}

#[test]
fn a_body_that_is_not_json_yields_an_empty_surface() {
    let surface = string_leaves(b"not json", SurfaceBudget::default());

    assert!(surface.is_empty());
    assert!(
        !surface.truncated(),
        "an unparseable body is the caller's to reject, not a truncated surface"
    );
}

#[test]
fn non_string_scalars_contribute_nothing() {
    let values = surface(&json!({ "n": 1, "b": true, "nil": null, "s": "kept" }));

    assert!(values.contains(&"kept".to_owned()));
    assert!(!values.iter().any(|v| v == "1" || v == "true"));
}

#[test]
fn sse_surface_spans_every_frame_of_a_stream() {
    let frames = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"delta\":{\"partial_json\":\"{\\\"token\\\":\\\"ghp_\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"delta\":{\"partial_json\":\"AAAABBBBCCCC\\\"}\"}}\n\n",
    );

    let surface = sse_string_leaves(frames.as_bytes(), SurfaceBudget::default());
    let values: Vec<&str> = surface.leaves().iter().map(|l| l.value.as_str()).collect();

    assert!(values.contains(&"msg_1"), "got {values:?}");
    assert!(
        values.iter().any(|v| v.contains("ghp_")),
        "the head of a value split across frames is present; got {values:?}"
    );
    assert!(
        values.iter().any(|v| v.contains("AAAABBBBCCCC")),
        "the tail of a value split across frames is present; got {values:?}"
    );
}

#[test]
fn a_malformed_frame_does_not_zero_the_rest_of_the_stream() {
    let frames = concat!(
        "data: {\"a\":\"before\"}\n\n",
        "data: {not json at all\n\n",
        "data: [DONE]\n\n",
        "data: {\"b\":\"after\"}\n\n",
    );

    let surface = sse_string_leaves(frames.as_bytes(), SurfaceBudget::default());
    let values: Vec<&str> = surface.leaves().iter().map(|l| l.value.as_str()).collect();

    assert!(values.contains(&"before"), "got {values:?}");
    assert!(values.contains(&"after"), "got {values:?}");
    assert!(
        !surface.truncated(),
        "an unparseable frame is skipped, not reported as a budget truncation"
    );
}

#[test]
fn multi_line_data_payloads_are_joined_before_parsing() {
    let frames = "data: {\"k\":\ndata: \"joined\"}\n\n";

    let surface = sse_string_leaves(frames.as_bytes(), SurfaceBudget::default());
    let values: Vec<&str> = surface.leaves().iter().map(|l| l.value.as_str()).collect();

    assert!(values.contains(&"joined"), "got {values:?}");
}
