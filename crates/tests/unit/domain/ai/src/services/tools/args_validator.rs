use serde_json::{Value, json};
use systemprompt_ai::models::{ArgsCheck, ArgsValidity};
use systemprompt_ai::services::tools::ToolArgsValidator;

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "lines": {"type": "array", "items": {"type": "integer"}}
        },
        "required": ["path"]
    })
}

fn validator() -> ToolArgsValidator {
    let read = schema();
    let broken = json!({"type": 12});
    ToolArgsValidator::compile([
        ("read", Some(&read)),
        ("broken", Some(&broken)),
        ("bare", None),
    ])
}

#[test]
fn valid_arguments_are_valid() {
    let check = validator().check_raw("read", r#"{"path":"/tmp/a","lines":[1,2]}"#);
    assert_eq!(check, ArgsCheck::of(ArgsValidity::Valid));
}

#[test]
fn invalid_arguments_carry_the_first_failing_pointer() {
    let check = validator().check_raw("read", r#"{"path":"/tmp/a","lines":[1,"two"]}"#);
    assert_eq!(check.validity, ArgsValidity::Invalid);
    assert_eq!(check.error_path.as_deref(), Some("/lines/1"));
}

#[test]
fn a_missing_required_field_points_at_the_root() {
    let check = validator().check_raw("read", "{}");
    assert_eq!(check.validity, ArgsValidity::Invalid);
    assert_eq!(check.error_path.as_deref(), Some(""));
}

#[test]
fn an_unoffered_tool_is_unknown() {
    let check = validator().check_raw("write", r#"{"path":"x"}"#);
    assert_eq!(check, ArgsCheck::of(ArgsValidity::UnknownTool));
}

#[test]
fn malformed_json_is_unparseable() {
    let check = validator().check_raw("read", r#"{"path":"x""#);
    assert_eq!(check, ArgsCheck::of(ArgsValidity::Unparseable));
}

#[test]
fn an_uncompilable_or_missing_schema_is_unusable() {
    let v = validator();
    assert_eq!(
        v.check_raw("broken", "{}"),
        ArgsCheck::of(ArgsValidity::SchemaUnusable)
    );
    assert_eq!(
        v.check("bare", &json!({})),
        ArgsCheck::of(ArgsValidity::SchemaUnusable)
    );
}

#[test]
fn empty_input_is_an_empty_object() {
    let open = json!({"type": "object"});
    let v = ToolArgsValidator::compile([("noargs", Some(&open))]);
    assert_eq!(
        v.check_raw("noargs", "  "),
        ArgsCheck::of(ArgsValidity::Valid)
    );
}

#[test]
fn first_definition_of_a_duplicate_name_wins() {
    let strict = json!({"type": "object", "required": ["a"]});
    let open = json!({"type": "object"});
    let v = ToolArgsValidator::compile([("t", Some(&strict)), ("t", Some(&open))]);
    assert_eq!(v.len(), 1);
    assert_eq!(v.check("t", &json!({})).validity, ArgsValidity::Invalid);
}

#[test]
fn validity_values_match_the_column_check() {
    let all = [
        (ArgsValidity::Valid, "valid"),
        (ArgsValidity::Invalid, "invalid"),
        (ArgsValidity::UnknownTool, "unknown_tool"),
        (ArgsValidity::Unparseable, "unparseable"),
        (ArgsValidity::SchemaUnusable, "schema_unusable"),
    ];
    for (validity, text) in all {
        assert_eq!(validity.as_str(), text);
        assert_eq!(
            serde_json::to_value(validity).expect("serialises"),
            json!(text)
        );
    }
}
