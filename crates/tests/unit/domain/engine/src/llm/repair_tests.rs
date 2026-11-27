use serde_json::json;
use systemprompt_engine::llm::{ToolInput, repair_tool_input};

#[test]
fn plain_object_passes_through() {
    let expected = ToolInput {
        value: json!({"path": "a.rs"}),
        malformed: None,
    };
    assert_eq!(repair_tool_input(r#"{"path":"a.rs"}"#), expected);
}

#[test]
fn empty_and_null_become_empty_object() {
    assert_eq!(repair_tool_input("  ").value, json!({}));
    assert_eq!(repair_tool_input("null").value, json!({}));
}

#[test]
fn string_encoded_json_is_unwrapped() {
    let raw = serde_json::to_string(&json!("{\"command\":\"ls\"}")).unwrap();
    assert_eq!(repair_tool_input(&raw).value, json!({"command": "ls"}));
}

#[test]
fn trailing_garbage_and_fences_are_dropped() {
    assert_eq!(
        repair_tool_input(r#"{"a":"}{"}garbage"#).value,
        json!({"a": "}{"})
    );
    assert_eq!(
        repair_tool_input("```json\n{\"a\":[1,2]}\n```").value,
        json!({"a": [1, 2]})
    );
    assert_eq!(
        repair_tool_input(r#"{"a":1}{"b":2}"#).value,
        json!({"a": 1})
    );
}

#[test]
fn unreadable_input_keeps_the_raw_text() {
    let out = repair_tool_input(r#"{"a": "unterminated"#);
    assert_eq!(out.value, json!({}));
    assert_eq!(out.malformed.as_deref(), Some(r#"{"a": "unterminated"#));
}
