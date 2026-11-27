use harness_tools::{ToolError, validate};
use serde_json::{Value, json};

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "offset": {"type": "integer", "minimum": 1},
            "ratio": {"type": "number"},
            "all": {"type": "boolean"},
            "mode": {"type": "string", "enum": ["a", "b"]},
            "items": {
                "type": "array",
                "items": {"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"]}
            }
        },
        "required": ["path"]
    })
}

#[test]
fn coerces_strings_and_drops_optional_nulls() {
    let mut input =
        json!({"path": "a", "offset": "12", "ratio": "0.5", "all": "TRUE", "mode": null});
    validate(&schema(), &mut input).unwrap();
    assert_eq!(
        input,
        json!({"path": "a", "offset": 12, "ratio": 0.5, "all": true})
    );
}

#[test]
fn parses_stringified_objects_and_arrays() {
    let mut input = Value::String(r#"{"path":"a","items":"[{\"n\":\"3\"}]"}"#.to_owned());
    validate(&schema(), &mut input).unwrap();
    assert_eq!(input, json!({"path": "a", "items": [{"n": 3}]}));
}

#[test]
fn whole_floats_become_integers() {
    let mut input = json!({"path": "a", "offset": 4.0});
    validate(&schema(), &mut input).unwrap();
    assert_eq!(input, json!({"path": "a", "offset": 4}));
    let mut fraction = json!({"path": "a", "offset": 4.5});
    assert!(matches!(
        validate(&schema(), &mut fraction),
        Err(ToolError::InvalidInput { message }) if message.contains("whole number")
    ));
}

#[test]
fn reports_missing_and_wrong_fields() {
    let mut missing = json!({"offset": 1});
    assert!(matches!(
        validate(&schema(), &mut missing),
        Err(ToolError::InvalidInput { message }) if message.contains("path")
    ));
    let mut low = json!({"path": "a", "offset": 0});
    assert!(matches!(
        validate(&schema(), &mut low),
        Err(ToolError::InvalidInput { message }) if message.contains("at least 1")
    ));
    let mut bad_enum = json!({"path": "a", "mode": "c"});
    assert!(matches!(
        validate(&schema(), &mut bad_enum),
        Err(ToolError::InvalidInput { message }) if message.contains("must be one of")
    ));
    let mut nested = json!({"path": "a", "items": [{}]});
    assert!(matches!(
        validate(&schema(), &mut nested),
        Err(ToolError::InvalidInput { message }) if message.contains("items[0].n")
    ));
}
