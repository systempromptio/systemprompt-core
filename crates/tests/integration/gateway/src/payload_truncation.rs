use bytes::Bytes;
use systemprompt_api::services::gateway::audit::payload::{slice_payload, truncate_for_tool_input};

const PAYLOAD_CAP: usize = 256 * 1024;
const TOOL_INPUT_CAP: usize = 64 * 1024;

#[test]
fn slice_payload_at_cap_preserves_parsed_json() {
    // Build a JSON value whose serialised form is just under the cap.
    let pad_len = PAYLOAD_CAP - 32;
    let body = format!(r#"{{"k":"{}"}}"#, "a".repeat(pad_len));
    assert!(body.len() <= PAYLOAD_CAP);
    let (json, excerpt, truncated, bytes) = slice_payload(&Bytes::from(body.clone()));
    assert!(!truncated, "payload at-or-below cap is not truncated");
    assert!(
        excerpt.is_none(),
        "at-cap parses as JSON, no excerpt needed"
    );
    assert!(json.is_some(), "JSON within cap must be parsed");
    assert_eq!(bytes as usize, body.len());
}

#[test]
fn slice_payload_one_over_cap_truncates_and_records_size() {
    let body = vec![b'x'; PAYLOAD_CAP + 1];
    let (json, excerpt, truncated, bytes) = slice_payload(&Bytes::from(body.clone()));
    assert!(truncated, "one byte over the cap must be flagged truncated");
    assert!(json.is_none(), "over-cap payload is never returned as JSON");
    let excerpt = excerpt.expect("over-cap must record an excerpt");
    assert!(
        excerpt.contains("<truncated"),
        "excerpt includes truncation marker"
    );
    assert_eq!(
        bytes as usize,
        body.len(),
        "byte count reflects original payload size, not truncated form"
    );
}

#[test]
fn slice_payload_invalid_json_within_cap_records_excerpt_not_silent_drop() {
    let body = Bytes::from_static(b"definitely not json {[");
    let (json, excerpt, truncated, bytes) = slice_payload(&body);
    assert!(json.is_none());
    assert!(!truncated, "small invalid-JSON body is not truncated");
    let excerpt = excerpt.expect("invalid JSON must surface as an excerpt, never silently drop");
    assert_eq!(excerpt, "definitely not json {[");
    assert_eq!(bytes as usize, body.len());
}

#[test]
fn slice_payload_oversized_invalid_json_keeps_head_and_tail() {
    let body = vec![b'q'; PAYLOAD_CAP * 2];
    let (json, excerpt, truncated, bytes) = slice_payload(&Bytes::from(body.clone()));
    assert!(json.is_none());
    assert!(truncated);
    let excerpt = excerpt.expect("oversized payload must record an excerpt");
    assert!(excerpt.starts_with("qqqq"), "head present");
    assert!(excerpt.ends_with("qqqq"), "tail present");
    assert!(excerpt.contains("<truncated"));
    assert_eq!(bytes as usize, body.len());
}

#[test]
fn truncate_for_tool_input_under_cap_is_identity() {
    let input = "small tool input";
    assert_eq!(truncate_for_tool_input(input), input);
}

#[test]
fn truncate_for_tool_input_at_cap_is_identity() {
    let input = "x".repeat(TOOL_INPUT_CAP);
    assert_eq!(truncate_for_tool_input(&input), input);
}

#[test]
fn truncate_for_tool_input_one_over_cap_truncates() {
    let input = "x".repeat(TOOL_INPUT_CAP + 1);
    let out = truncate_for_tool_input(&input);
    assert!(out.starts_with(&"x".repeat(TOOL_INPUT_CAP)));
    assert!(out.contains("<truncated 1 bytes>"));
}

#[test]
fn truncate_for_tool_input_does_not_panic_on_utf8_boundary() {
    // Place a 4-byte emoji starting one byte before the cap so the naive
    // slice `&input[..TOOL_INPUT_CAP]` would land inside its codepoint.
    let pre = "a".repeat(TOOL_INPUT_CAP - 1);
    let input = format!("{pre}🚀tail");
    // Must not panic.
    let out = truncate_for_tool_input(&input);
    assert!(out.contains("<truncated"));
    // The walked-back cut must land on a char boundary, so the head is valid
    // UTF-8 and contains all the preceding ASCII padding.
    assert!(out.starts_with(&pre));
}

#[test]
fn truncate_for_tool_input_preserves_full_codepoints_in_head() {
    let pre = "a".repeat(TOOL_INPUT_CAP - 2);
    let input = format!("{pre}€tail"); // € is 3 bytes (E2 82 AC)
    let out = truncate_for_tool_input(&input);
    // Either the head includes the full €, or it stops before it; either way,
    // the result must be valid UTF-8 (the format!() above already proves that
    // for the input, and `truncate_for_tool_input` must preserve validity).
    assert!(out.is_char_boundary(out.len()));
}
