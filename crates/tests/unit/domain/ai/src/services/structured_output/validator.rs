//! Tests for SchemaValidator.

use systemprompt_core_ai::services::structured_output::validator::SchemaValidator;
use serde_json::json;

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

        // Non-strict mode should allow extra properties
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

mod string_validation_tests {
    use super::*;

    #[test]
    fn validates_min_length() {
        let value = json!("hello");
        let schema = json!({
            "type": "string",
            "minLength": 3
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_below_min_length() {
        let value = json!("hi");
        let schema = json!({
            "type": "string",
            "minLength": 3
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_max_length() {
        let value = json!("hello");
        let schema = json!({
            "type": "string",
            "maxLength": 10
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_above_max_length() {
        let value = json!("hello world");
        let schema = json!({
            "type": "string",
            "maxLength": 5
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_pattern() {
        let value = json!("test@example.com");
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+@[a-z]+\\.[a-z]+$"
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_non_matching_pattern() {
        let value = json!("not an email");
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+@[a-z]+\\.[a-z]+$"
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }
}

mod number_validation_tests {
    use super::*;

    #[test]
    fn validates_minimum() {
        let value = json!(10);
        let schema = json!({
            "type": "number",
            "minimum": 5
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_below_minimum() {
        let value = json!(3);
        let schema = json!({
            "type": "number",
            "minimum": 5
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_maximum() {
        let value = json!(10);
        let schema = json!({
            "type": "number",
            "maximum": 100
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_above_maximum() {
        let value = json!(150);
        let schema = json!({
            "type": "number",
            "maximum": 100
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }
}

mod enum_validation_tests {
    use super::*;

    #[test]
    fn validates_enum_value() {
        let value = json!("active");
        let schema = json!({
            "enum": ["active", "inactive", "pending"]
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn rejects_non_enum_value() {
        let value = json!("unknown");
        let schema = json!({
            "enum": ["active", "inactive", "pending"]
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_err());
    }

    #[test]
    fn validates_numeric_enum() {
        let value = json!(2);
        let schema = json!({
            "enum": [1, 2, 3]
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_mixed_enum() {
        let schema = json!({
            "enum": [1, "two", null, true]
        });

        assert!(SchemaValidator::validate(&json!(1), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!("two"), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!(null), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!(true), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!("other"), &schema, true).is_err());
    }
}

mod nested_validation_tests {
    use super::*;

    #[test]
    fn validates_nested_objects() {
        let value = json!({
            "user": {
                "name": "Alice",
                "age": 30
            }
        });
        let schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "age": {"type": "integer"}
                    },
                    "required": ["name"]
                }
            }
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn validates_array_of_objects() {
        let value = json!([
            {"id": 1, "name": "Item 1"},
            {"id": 2, "name": "Item 2"}
        ]);
        let schema = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"}
                },
                "required": ["id", "name"]
            }
        });

        assert!(SchemaValidator::validate(&value, &schema, true).is_ok());
    }

    #[test]
    fn path_included_in_error() {
        let value = json!({
            "users": [
                {"name": "Alice"},
                {"name": 123}  // Invalid
            ]
        });
        let schema = json!({
            "type": "object",
            "properties": {
                "users": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"}
                        }
                    }
                }
            }
        });

        let result = SchemaValidator::validate(&value, &schema, true);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        // Should contain path info
        assert!(error.contains("users") || error.contains("[1]"));
    }
}

mod no_type_validation_tests {
    use super::*;

    #[test]
    fn accepts_any_value_without_type() {
        let schema = json!({});

        assert!(SchemaValidator::validate(&json!("string"), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!(42), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!(null), &schema, true).is_ok());
        assert!(SchemaValidator::validate(&json!([1, 2]), &schema, true).is_ok());
    }
}
