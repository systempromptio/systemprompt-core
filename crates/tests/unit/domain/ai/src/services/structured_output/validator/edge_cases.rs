use serde_json::json;
use systemprompt_ai::services::structured_output::validator::SchemaValidator;

mod boolean_validation_tests {
    use super::*;

    #[test]
    fn rejects_string_as_boolean() {
        let value = json!("true");
        let schema = json!({"type": "boolean"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn rejects_number_as_boolean() {
        let value = json!(1);
        let schema = json!({"type": "boolean"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_false() {
        let value = json!(false);
        let schema = json!({"type": "boolean"});

        SchemaValidator::validate(&value, &schema, true).expect("false should be valid boolean");
    }
}

mod null_validation_tests {
    use super::*;

    #[test]
    fn rejects_string_as_null() {
        let value = json!("null");
        let schema = json!({"type": "null"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn rejects_zero_as_null() {
        let value = json!(0);
        let schema = json!({"type": "null"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn rejects_false_as_null() {
        let value = json!(false);
        let schema = json!({"type": "null"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn rejects_empty_string_as_null() {
        let value = json!("");
        let schema = json!({"type": "null"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }
}

mod integer_validation_tests {
    use super::*;

    #[test]
    fn rejects_float_as_integer() {
        let value = json!(3.14);
        let schema = json!({"type": "integer"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_negative_integer() {
        let value = json!(-42);
        let schema = json!({"type": "integer"});

        SchemaValidator::validate(&value, &schema, true).expect("negative integer should pass");
    }

    #[test]
    fn validates_zero() {
        let value = json!(0);
        let schema = json!({"type": "integer"});

        SchemaValidator::validate(&value, &schema, true).expect("zero should pass");
    }

    #[test]
    fn validates_integer_with_min_max() {
        let value = json!(5);
        let schema = json!({
            "type": "integer",
            "minimum": 1,
            "maximum": 10
        });

        SchemaValidator::validate(&value, &schema, true).expect("5 in [1,10] should pass");
    }

    #[test]
    fn rejects_integer_below_minimum() {
        let value = json!(0);
        let schema = json!({
            "type": "integer",
            "minimum": 1
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }
}

mod complex_nested_tests {
    use super::*;

    #[test]
    fn validates_object_in_array_in_object() {
        let value = json!({
            "data": {
                "items": [
                    {"name": "Alice", "score": 95},
                    {"name": "Bob", "score": 87}
                ]
            }
        });
        let schema = json!({
            "type": "object",
            "properties": {
                "data": {
                    "type": "object",
                    "properties": {
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": {"type": "string"},
                                    "score": {"type": "integer"}
                                },
                                "required": ["name", "score"]
                            }
                        }
                    }
                }
            }
        });

        SchemaValidator::validate(&value, &schema, true).expect("complex nested should pass");
    }

    #[test]
    fn rejects_deeply_nested_type_error() {
        let value = json!({
            "level1": {
                "level2": {
                    "value": "not a number"
                }
            }
        });
        let schema = json!({
            "type": "object",
            "properties": {
                "level1": {
                    "type": "object",
                    "properties": {
                        "level2": {
                            "type": "object",
                            "properties": {
                                "value": {"type": "number"}
                            }
                        }
                    }
                }
            }
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn empty_object_passes_no_required() {
        let value = json!({});
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        SchemaValidator::validate(&value, &schema, true).expect("empty object with optional props should pass");
    }

    #[test]
    fn empty_array_passes_validation() {
        let value = json!([]);
        let schema = json!({
            "type": "array",
            "items": {"type": "string"}
        });

        SchemaValidator::validate(&value, &schema, true).expect("empty array should pass");
    }

    #[test]
    fn empty_array_fails_min_items() {
        let value = json!([]);
        let schema = json!({
            "type": "array",
            "minItems": 1
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn multiple_required_fields_missing() {
        let value = json!({});
        let schema = json!({
            "type": "object",
            "required": ["a", "b", "c"]
        });

        let err = SchemaValidator::validate(&value, &schema, true).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("a") || msg.contains("b") || msg.contains("c"));
    }
}

mod string_edge_cases {
    use super::*;

    #[test]
    fn empty_string_passes_no_constraints() {
        let value = json!("");
        let schema = json!({"type": "string"});

        SchemaValidator::validate(&value, &schema, true).expect("empty string should pass");
    }

    #[test]
    fn empty_string_fails_min_length_1() {
        let value = json!("");
        let schema = json!({
            "type": "string",
            "minLength": 1
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn exact_min_length_passes() {
        let value = json!("abc");
        let schema = json!({
            "type": "string",
            "minLength": 3
        });

        SchemaValidator::validate(&value, &schema, true).expect("exact min length should pass");
    }

    #[test]
    fn exact_max_length_passes() {
        let value = json!("abc");
        let schema = json!({
            "type": "string",
            "maxLength": 3
        });

        SchemaValidator::validate(&value, &schema, true).expect("exact max length should pass");
    }
}

mod number_edge_cases {
    use super::*;

    #[test]
    fn exact_minimum_passes() {
        let value = json!(5);
        let schema = json!({
            "type": "number",
            "minimum": 5
        });

        SchemaValidator::validate(&value, &schema, true).expect("exact minimum should pass");
    }

    #[test]
    fn exact_maximum_passes() {
        let value = json!(10);
        let schema = json!({
            "type": "number",
            "maximum": 10
        });

        SchemaValidator::validate(&value, &schema, true).expect("exact maximum should pass");
    }

    #[test]
    fn negative_number_with_minimum() {
        let value = json!(-5);
        let schema = json!({
            "type": "number",
            "minimum": -10
        });

        SchemaValidator::validate(&value, &schema, true).expect("-5 >= -10 should pass");
    }

    #[test]
    fn float_precision_validation() {
        let value = json!(3.14159);
        let schema = json!({
            "type": "number",
            "minimum": 3.14,
            "maximum": 3.15
        });

        SchemaValidator::validate(&value, &schema, true).expect("pi in range should pass");
    }
}
