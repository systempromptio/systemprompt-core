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

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_below_min_length() {
        let value = json!("hi");
        let schema = json!({
            "type": "string",
            "minLength": 3
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_max_length() {
        let value = json!("hello");
        let schema = json!({
            "type": "string",
            "maxLength": 10
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_above_max_length() {
        let value = json!("hello world");
        let schema = json!({
            "type": "string",
            "maxLength": 5
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_pattern() {
        let value = json!("test@example.com");
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+@[a-z]+\\.[a-z]+$"
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_non_matching_pattern() {
        let value = json!("not an email");
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+@[a-z]+\\.[a-z]+$"
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
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

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_below_minimum() {
        let value = json!(3);
        let schema = json!({
            "type": "number",
            "minimum": 5
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_maximum() {
        let value = json!(10);
        let schema = json!({
            "type": "number",
            "maximum": 100
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_above_maximum() {
        let value = json!(150);
        let schema = json!({
            "type": "number",
            "maximum": 100
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
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

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn rejects_non_enum_value() {
        let value = json!("unknown");
        let schema = json!({
            "enum": ["active", "inactive", "pending"]
        });

        SchemaValidator::validate(&value, &schema, true).unwrap_err();
    }

    #[test]
    fn validates_numeric_enum() {
        let value = json!(2);
        let schema = json!({
            "enum": [1, 2, 3]
        });

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
    }

    #[test]
    fn validates_mixed_enum() {
        let schema = json!({
            "enum": [1, "two", null, true]
        });

        SchemaValidator::validate(&json!(1), &schema, true).expect("1 should match enum");
        SchemaValidator::validate(&json!("two"), &schema, true).expect("two should match enum");
        SchemaValidator::validate(&json!(null), &schema, true).expect("null should match enum");
        SchemaValidator::validate(&json!(true), &schema, true).expect("true should match enum");
        SchemaValidator::validate(&json!("other"), &schema, true).unwrap_err();
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

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
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

        SchemaValidator::validate(&value, &schema, true).expect("validation should pass");
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

        let error = SchemaValidator::validate(&value, &schema, true).unwrap_err().to_string();
        assert!(error.contains("users") || error.contains("[1]"));
    }
}

mod no_type_validation_tests {
    use super::*;

    #[test]
    fn accepts_any_value_without_type() {
        let schema = json!({});

        SchemaValidator::validate(&json!("string"), &schema, true).expect("string should pass");
        SchemaValidator::validate(&json!(42), &schema, true).expect("number should pass");
        SchemaValidator::validate(&json!(null), &schema, true).expect("null should pass");
        SchemaValidator::validate(&json!([1, 2]), &schema, true).expect("array should pass");
    }
}
