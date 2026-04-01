//! Tests for type validation, object validation, and array validation

use serde_json::json;
use systemprompt_ai::services::structured_output::validator::SchemaValidator;

mod type_validation_tests {
    use super::*;

    #[test]
    fn validates_string_type() {
        let value = json!("hello");
        let schema = json!({"type": "string"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_wrong_string_type() {
        let value = json!(42);
        let schema = json!({"type": "string"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_number_type() {
        let value = json!(3.14);
        let schema = json!({"type": "number"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_integer_type() {
        let value = json!(42);
        let schema = json!({"type": "integer"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_boolean_type() {
        let value = json!(true);
        let schema = json!({"type": "boolean"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_null_type() {
        let value = json!(null);
        let schema = json!({"type": "null"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_array_type() {
        let value = json!([1, 2, 3]);
        let schema = json!({"type": "array"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_object_type() {
        let value = json!({"key": "value"});
        let schema = json!({"type": "object"});

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_multiple_types() {
        let schema = json!({"type": ["string", "null"]});

        assert!(SchemaValidator::validate(&json!("test"), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!(null), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!(42), &schema, true).is_err());
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

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_missing_required_properties() {
        let value = json!({"other": "field"});
        let schema = json!({
            "type": "object",
            "required": ["name"]
        });

        let result = SchemaValidator::validate(&value, &schema, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name"));
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

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
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

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
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

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
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

        assert!(SchemaValidator::validate(&value, &schema, false).is_ok());
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

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
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

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_below_min_items() {
        let value = json!([1]);
        let schema = json!({
            "type": "array",
            "minItems": 2
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_max_items() {
        let value = json!([1, 2]);
        let schema = json!({
            "type": "array",
            "maxItems": 5
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_above_max_items() {
        let value = json!([1, 2, 3, 4, 5, 6]);
        let schema = json!({
            "type": "array",
            "maxItems": 5
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_item_types() {
        let value = json!([1, 2, 3]);
        let schema = json!({
            "type": "array",
            "items": {"type": "integer"}
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_wrong_item_type() {
        let value = json!([1, "two", 3]);
        let schema = json!({
            "type": "array",
            "items": {"type": "integer"}
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }
}
