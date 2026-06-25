//! Unit tests for the gateway audit capture types.

use systemprompt_api::services::gateway::{CapturedToolUse, CapturedUsage};

#[test]
fn captured_usage_default_is_zero() {
    let u = CapturedUsage::default();
    assert_eq!(u.input_tokens, 0);
    assert_eq!(u.output_tokens, 0);
}

#[test]
fn captured_usage_is_copy_and_clone() {
    let u = CapturedUsage {
        input_tokens: 10,
        output_tokens: 20,
        cache_read_tokens: 5,
        cache_creation_tokens: 0,
    };
    let copy = u;
    let cloned = u;
    assert_eq!(copy.input_tokens, 10);
    assert_eq!(cloned.output_tokens, 20);
    assert_eq!(copy.cache_read_tokens, 5);
    assert_eq!(u.input_tokens, 10);
}

#[test]
fn captured_tool_use_is_cloneable() {
    let t = CapturedToolUse {
        ai_tool_call_id: "tu_1".into(),
        tool_name: "search".into(),
        tool_input: "{\"q\":\"x\"}".into(),
    };
    let cloned = t.clone();
    assert_eq!(cloned.tool_name, "search");
    assert_eq!(cloned.ai_tool_call_id, "tu_1");
    assert_eq!(cloned.tool_input, "{\"q\":\"x\"}");
}

#[test]
fn captured_usage_debug_renders_fields() {
    let u = CapturedUsage {
        input_tokens: 1,
        output_tokens: 2,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
    };
    let s = format!("{u:?}");
    assert!(s.contains("CapturedUsage"));
    assert!(s.contains("input_tokens"));
}
