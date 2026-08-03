//! Type-union schemas, the non-string `type` fallthrough, and the
//! wrong-JSON-kind rejection each validator arm opens with.

use serde_json::json;
use systemprompt_ai::services::structured_output::validator::SchemaValidator;

#[test]
fn a_union_type_accepts_any_listed_member_and_rejects_the_rest() {
    let schema = json!({"type": ["string", "null"]});

    SchemaValidator::validate(&json!("text"), &schema, false).expect("string member accepted");
    SchemaValidator::validate(&json!(null), &schema, false).expect("null member accepted");

    let err = SchemaValidator::validate(&json!(42), &schema, false)
        .expect_err("a number is in neither branch of the union");
    let message = err.to_string();
    assert!(
        message.contains("root") && message.contains("number"),
        "the mismatch must name the path and the actual JSON kind, got {message}"
    );
}

#[test]
fn a_type_keyword_that_is_neither_a_string_nor_an_array_constrains_nothing() {
    let schema = json!({"type": {"unexpected": "shape"}});

    SchemaValidator::validate(&json!("anything"), &schema, false)
        .expect("an uninterpretable type keyword must not reject a value");
    SchemaValidator::validate(&json!([1, 2, 3]), &schema, false)
        .expect("an uninterpretable type keyword must not reject a value");
}

#[test]
fn an_unrecognised_type_name_is_treated_as_unconstrained() {
    let schema = json!({"type": "geometry"});

    SchemaValidator::validate(&json!({"x": 1}), &schema, false)
        .expect("an unknown type name must not be enforced as a mismatch");
}

#[test]
fn every_typed_arm_rejects_a_value_of_the_wrong_json_kind_by_path() {
    for (type_name, wrong_value, actual_kind) in [
        ("object", json!("not an object"), "string"),
        ("array", json!("not an array"), "string"),
        ("string", json!(1), "number"),
        ("number", json!("nope"), "string"),
        ("boolean", json!("nope"), "string"),
        ("null", json!("nope"), "string"),
    ] {
        let err = SchemaValidator::validate(&wrong_value, &json!({"type": type_name}), false)
            .expect_err("a wrong-kind value must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("root"),
            "the {type_name} arm must report the failing path, got {message}"
        );
        assert!(
            message.contains(type_name) && message.contains(actual_kind),
            "the {type_name} arm must name both the expected and the actual kind, got {message}"
        );
    }
}

#[test]
fn a_nested_failure_reports_the_full_path_to_the_offending_element() {
    let schema = json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {"label": {"type": "string", "minLength": 3}},
                    "required": ["label"]
                }
            }
        }
    });

    let err = SchemaValidator::validate(
        &json!({"items": [{"label": "okay"}, {"label": "x"}]}),
        &schema,
        false,
    )
    .expect_err("the second element violates minLength");
    let message = err.to_string();
    assert!(
        message.contains("items[1]") && message.contains("label"),
        "the path must locate the element and property, got {message}"
    );

    let missing = SchemaValidator::validate(&json!({"items": [{}]}), &schema, false)
        .expect_err("the element is missing its required property");
    assert!(
        missing.to_string().contains("items[0]"),
        "a missing required property must be located too, got {missing}"
    );
}
