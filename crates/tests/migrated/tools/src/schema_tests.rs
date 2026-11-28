//! Repairing and checking tool input against its JSON Schema.

use serde_json::{Value, json};
use systemprompt_engine::tools::{ToolError, validate};

fn repaired(schema: &Value, input: Value) -> Result<Value, ToolError> {
    let mut input = input;
    validate(schema, &mut input).map(|()| input)
}

#[test]
#[ignore = "parity ledger: unknown keys are kept even when the object schema is closed"]
fn unknown_keys_are_removed_only_from_closed_objects() {
    let closed = json!({"type": "object", "properties": {"a": {"type": "string"}}, "additionalProperties": false});
    assert_eq!(
        repaired(&closed, json!({"a": "x", "extra": 1})).unwrap(),
        json!({"a": "x"})
    );
}

#[test]
fn unknown_keys_of_open_patterned_or_composed_objects_are_kept() {
    let open = json!({"type": "object", "properties": {"a": {"type": "string"}}});
    assert_eq!(
        repaired(&open, json!({"a": "x", "extra": 1})).unwrap(),
        json!({"a": "x", "extra": 1})
    );
    let patterned = json!({"type": "object", "patternProperties": {"^x-": {"type": "string"}}, "additionalProperties": false});
    assert_eq!(
        repaired(&patterned, json!({"x-a": "1", "y": 2})).unwrap(),
        json!({"x-a": "1", "y": 2})
    );
    let composed = json!({"type": "object", "properties": {"a": {"type": "string"}}, "allOf": [{"properties": {"b": {"type": "string"}}}]});
    assert_eq!(
        repaired(&composed, json!({"a": "x", "b": "y"})).unwrap(),
        json!({"a": "x", "b": "y"})
    );
}

#[test]
fn an_optional_null_is_removed_and_a_required_one_is_reported() {
    let schema = json!({"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "string"}}, "required": ["a"]});
    assert_eq!(
        repaired(&schema, json!({"a": "x", "b": null})).unwrap(),
        json!({"a": "x"})
    );
    let err = repaired(&schema, json!({"a": null})).unwrap_err();
    assert_eq!(err.to_string(), "invalid input: `a` must be a string");
}

#[test]
#[ignore = "parity ledger: an optional null is removed even where the schema allows null"]
fn a_null_the_schema_allows_is_kept() {
    let nullable = json!({"type": "object", "properties": {"a": {"type": ["string", "null"]}}});
    assert_eq!(
        repaired(&nullable, json!({"a": null})).unwrap(),
        json!({"a": null})
    );
    let composed = json!({"type": "object", "properties": {"a": {"anyOf": [{"type": "string"}, {"type": "null"}]}}});
    assert_eq!(
        repaired(&composed, json!({"a": null})).unwrap(),
        json!({"a": null})
    );
}

#[test]
#[ignore = "parity ledger: a lone value is not wrapped into the array its schema asks for"]
fn a_lone_compatible_value_is_wrapped_into_an_array() {
    let schema = json!({"type": "object", "properties": {"paths": {"type": "array", "items": {"type": "string"}}}});
    assert_eq!(
        repaired(&schema, json!({"paths": "a.rs"})).unwrap(),
        json!({"paths": ["a.rs"]})
    );
}

#[test]
fn array_items_are_repaired_in_place() {
    let schema = json!({"type": "object", "properties": {"n": {"type": "array", "items": {"type": "integer"}}}});
    assert_eq!(
        repaired(&schema, json!({"n": ["1", 2.0, 3]})).unwrap(),
        json!({"n": [1, 2, 3]})
    );
    let err = repaired(&schema, json!({"n": ["x"]})).unwrap_err();
    assert!(err.to_string().contains("n[0]"), "{err}");
}

#[test]
#[ignore = "parity ledger: tuple positions, typed maps and local references are not repaired"]
fn tuple_positions_typed_maps_and_local_references_are_repaired() {
    let tuple = json!({"type": "array", "prefixItems": [{"type": "integer"}, {"type": "boolean"}]});
    assert_eq!(
        repaired(&tuple, json!(["1", "true"])).unwrap(),
        json!([1, true])
    );
    let map = json!({"type": "object", "additionalProperties": {"type": "integer"}});
    assert_eq!(repaired(&map, json!({"a": "2"})).unwrap(), json!({"a": 2}));
    let reference = json!({"type": "object", "properties": {"n": {"$ref": "#/$defs/n"}}, "$defs": {"n": {"type": "integer"}}});
    assert_eq!(
        repaired(&reference, json!({"n": "3"})).unwrap(),
        json!({"n": 3})
    );
}

#[test]
fn ambiguous_unions_and_compositions_are_left_unchanged() {
    let schema = json!({"type": "object", "properties": {
        "a": {"anyOf": [{"type": "integer"}, {"type": "string"}]},
        "b": {"oneOf": [{"type": "boolean"}, {"type": "string"}]},
        "c": {"allOf": [{"type": "integer"}]},
    }});
    let input = json!({"a": "1", "b": "true", "c": "2"});
    assert_eq!(repaired(&schema, input.clone()).unwrap(), input);
}

#[test]
fn input_that_fails_its_schema_is_a_failure_naming_the_field() {
    let schema = json!({"type": "object", "properties": {"n": {"type": "integer", "minimum": 1}}, "required": ["n"]});
    assert_eq!(
        repaired(&schema, json!({})).unwrap_err().to_string(),
        "invalid input: `n` is required"
    );
    assert_eq!(
        repaired(&schema, json!({"n": 0})).unwrap_err().to_string(),
        "invalid input: `n` must be at least 1"
    );
    assert!(matches!(
        repaired(&schema, json!({"n": "x"})).unwrap_err(),
        ToolError::InvalidInput { .. }
    ));
}

#[test]
#[ignore = "parity ledger: definitions referenced from an external schema are not resolved"]
fn an_external_schema_with_draft_definitions_validates_input() {
    let schema = json!({"type": "object", "properties": {"n": {"$ref": "#/definitions/n"}}, "definitions": {"n": {"type": "integer"}}});
    assert!(repaired(&schema, json!({"n": "nope"})).is_err());
}

#[test]
fn keywords_outside_the_supported_subset_pass_input_through() {
    let schema = json!({"type": "object", "properties": {"s": {"type": "string", "pattern": "^a", "format": "uri"}}});
    let input = json!({"s": "zzz"});
    assert_eq!(repaired(&schema, input.clone()).unwrap(), input);
}

#[test]
fn a_tool_without_an_input_schema_takes_an_empty_object() {
    let empty = json!({"type": "object"});
    assert_eq!(repaired(&empty, Value::Null).unwrap(), json!({}));
    assert_eq!(
        repaired(&json!({}), json!({"anything": 1})).unwrap(),
        json!({"anything": 1})
    );
}
