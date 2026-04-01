//! Tests for type validation, object validation, and array validation

use serde_json::json;
use systemprompt_ai::services::structured_output::validator::SchemaValidator;

mod type_validation_tests {
    use super::*;

    #[test]
    fn validates_string_type() {
        let value = json!("hello");
        let schema = json!({"type": "string"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_wrong_string_type() {
        let value = json!(42);
        let schema = json!({"type": "string"});

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_number_type() {
        let value = json!(3.14);
        let schema = json!({"type": "number"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_integer_type() {
        let value = json!(42);
        let schema = json!({"type": "integer"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_boolean_type() {
        let value = json!(true);
        let schema = json!({"type": "boolean"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_null_type() {
        let value = json!(null);
        let schema = json!({"type": "null"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_array_type() {
        let value = json!([1, 2, 3]);
        let schema = json!({"type": "array"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_object_type() {
        let value = json!({"key": "value"});
        let schema = json!({"type": "object"});

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_multiple_types() {
        let schema = json!({"type": ["string", "null"]});

        SchemaValidator::validate(&json!("test"), &schema, true).expect("string should match");
        SchemaValidator::validate(&json!(null), &schema, true).expect("null should match");
        SchemaValidator::validate(&json!(42), &schema, true).unwrap_err();
    }
}

mod object_validation_tests {
    use super::*;

    #[test]
    fn validates_required_properties() {
        let value = json!({"name": "Alice"});
        let schema = json!({
            "type": "object",
            "required": ["name"]
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_missing_required_properties() {
        let value = json!({"other": "field"});
        let schema = json!({
            "type": "object",
            "required": ["name"]
        });

        let err = SchemaValidator::validate(&value, &schema, true).unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn validates_property_types() {
        let value = json!({"count": 42});
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer"}
            }
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_wrong_property_type() {
        let value = json!({"count": "not a number"});
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer"}
            }
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn rejects_additional_properties_in_strict_mode() {
        let value = json!({"name": "Alice", "extra": "field"});
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "additionalProperties": false
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn allows_additional_properties_in_non_strict_mode() {
        let value = json!({"name": "Alice", "extra": "field"});
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "additionalProperties": false
        });

        SchemaValidator::validate(&value, &schema, false).expect("non-strict mode should allow additional properties");
    }

    #[test]
    fn allows_additional_properties_by_default() {
        let value = json!({"name": "Alice", "extra": "field"});
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }
}

mod array_validation_tests {
    use super::*;

    #[test]
    fn validates_min_items() {
        let value = json!([1, 2, 3]);
        let schema = json!({
            "type": "array",
            "minItems": 2
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_below_min_items() {
        let value = json!([1]);
        let schema = json!({
            "type": "array",
            "minItems": 2
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_max_items() {
        let value = json!([1, 2]);
        let schema = json!({
            "type": "array",
            "maxItems": 5
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_above_max_items() {
        let value = json!([1, 2, 3, 4, 5, 6]);
        let schema = json!({
            "type": "array",
            "maxItems": 5
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_item_types() {
        let value = json!([1, 2, 3]);
        let schema = json!({
            "type": "array",
            "items": {"type": "integer"}
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_wrong_item_type() {
        let value = json!([1, "two", 3]);
        let schema = json!({
            "type": "array",
            "items": {"type": "integer"}
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }
}
