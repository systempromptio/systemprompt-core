//! Tests for string validation, number validation, enum validation, nested validation, and no-type validation

use serde_json::json;
use systemprompt_ai::services::structured_output::validator::SchemaValidator;

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
                {"name": 123}
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
