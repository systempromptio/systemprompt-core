use serde_json::json;
use systemprompt_ai::services::schema::DiscriminatedUnion;

mod edge_case_tests {
    use super::*;

    #[test]
    fn detect_returns_none_for_null() {
        let schema = json!(null);
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_string() {
        let schema = json!("not an object");
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_number() {
        let schema = json!(42);
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_boolean() {
        let schema = json!(true);
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_allof_with_non_object_items() {
        let schema = json!({
            "allOf": ["not an object"]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_allof_non_array() {
        let schema = json!({
            "allOf": "not an array"
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_missing_properties_in_if() {
        let schema = json!({
            "allOf": [
                {
                    "if": {},
                    "then": {"properties": {"data": {"type": "string"}}}
                }
            ]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_non_string_const() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"const": 42}
                        }
                    },
                    "then": {}
                }
            ]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_empty_if_properties() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {}
                    },
                    "then": {}
                }
            ]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_if_properties_non_object() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": "not an object"
                    },
                    "then": {}
                }
            ]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn base_properties_default_to_empty_object() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "type": {"const": "a"}
                        }
                    },
                    "then": {
                        "properties": {
                            "data": {"type": "string"}
                        }
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema).unwrap();
        assert!(result.base_properties.is_object());
        assert!(result.base_properties.as_object().unwrap().is_empty());
    }

    #[test]
    fn three_variants_detected() {
        let schema = json!({
            "allOf": [
                {
                    "if": {"properties": {"op": {"const": "add"}}},
                    "then": {"properties": {"a": {"type": "number"}, "b": {"type": "number"}}}
                },
                {
                    "if": {"properties": {"op": {"const": "sub"}}},
                    "then": {"properties": {"a": {"type": "number"}, "b": {"type": "number"}}}
                },
                {
                    "if": {"properties": {"op": {"const": "mul"}}},
                    "then": {"properties": {"a": {"type": "number"}, "b": {"type": "number"}}}
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema).unwrap();
        assert_eq!(result.discriminator_field, "op");
        assert_eq!(result.discriminator_values.len(), 3);
        assert_eq!(result.variants.len(), 3);
    }

    #[test]
    fn discriminator_field_preserved() {
        let schema = json!({
            "allOf": [
                {
                    "if": {"properties": {"command_type": {"const": "execute"}}},
                    "then": {"properties": {"script": {"type": "string"}}}
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema).unwrap();
        assert_eq!(result.discriminator_field, "command_type");
    }

    #[test]
    fn if_clause_non_object_returns_none() {
        let schema = json!({
            "allOf": [
                {
                    "if": "not an object",
                    "then": {}
                }
            ]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn discriminator_value_missing_const_field() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"type": "string"}
                        }
                    },
                    "then": {}
                }
            ]
        });
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }
}
