//! Tests for DiscriminatedUnion detection and analysis.

use systemprompt_core_ai::services::schema::DiscriminatedUnion;
use serde_json::json;

mod discriminated_union_tests {
    use super::*;

    #[test]
    fn detect_returns_none_for_simple_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_non_object() {
        let schema = json!({"type": "string"});
        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_empty_allof() {
        let schema = json!({
            "allOf": []
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_valid_discriminated_union() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            },
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"const": "create"}
                        }
                    },
                    "then": {
                        "properties": {
                            "data": {"type": "string"}
                        }
                    }
                },
                {
                    "if": {
                        "properties": {
                            "action": {"const": "delete"}
                        }
                    },
                    "then": {
                        "properties": {
                            "id": {"type": "integer"}
                        }
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_some());

        let union = result.unwrap();
        assert_eq!(union.discriminator_field, "action");
        assert_eq!(union.discriminator_values.len(), 2);
        assert!(union.discriminator_values.contains(&"create".to_string()));
        assert!(union.discriminator_values.contains(&"delete".to_string()));
        assert_eq!(union.variants.len(), 2);
    }

    #[test]
    fn detect_returns_none_for_inconsistent_discriminator() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"const": "create"}
                        }
                    },
                    "then": {}
                },
                {
                    "if": {
                        "properties": {
                            "type": {"const": "delete"}  // Different field
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
    fn detect_returns_none_for_missing_if_clause() {
        let schema = json!({
            "allOf": [
                {
                    "then": {
                        "properties": {"data": {"type": "string"}}
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_missing_then_clause() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"const": "create"}
                        }
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_missing_const() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "action": {"type": "string"}  // No const
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
    fn base_properties_captured() {
        let schema = json!({
            "properties": {
                "commonField": {"type": "string"},
                "anotherField": {"type": "boolean"}
            },
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "type": {"const": "variant1"}
                        }
                    },
                    "then": {
                        "properties": {
                            "variant1Field": {"type": "number"}
                        }
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_some());

        let union = result.unwrap();
        let base = union.base_properties.as_object().unwrap();
        assert!(base.contains_key("commonField"));
        assert!(base.contains_key("anotherField"));
    }

    #[test]
    fn variants_stored_correctly() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "mode": {"const": "simple"}
                        }
                    },
                    "then": {
                        "properties": {
                            "value": {"type": "string"}
                        },
                        "required": ["value"]
                    }
                },
                {
                    "if": {
                        "properties": {
                            "mode": {"const": "advanced"}
                        }
                    },
                    "then": {
                        "properties": {
                            "values": {"type": "array"}
                        }
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema);
        let union = result.unwrap();

        assert!(union.variants.contains_key("simple"));
        assert!(union.variants.contains_key("advanced"));

        let simple_variant = &union.variants["simple"];
        assert!(simple_variant["properties"]["value"].is_object());
    }

    #[test]
    fn single_variant_union() {
        let schema = json!({
            "allOf": [
                {
                    "if": {
                        "properties": {
                            "type": {"const": "only_option"}
                        }
                    },
                    "then": {
                        "properties": {
                            "data": {"type": "object"}
                        }
                    }
                }
            ]
        });

        let result = DiscriminatedUnion::detect(&schema);
        assert!(result.is_some());

        let union = result.unwrap();
        assert_eq!(union.discriminator_values.len(), 1);
        assert_eq!(union.discriminator_values[0], "only_option");
    }
}
