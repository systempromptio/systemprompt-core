//! Tests for ProviderCapabilities.

use systemprompt_core_ai::services::schema::ProviderCapabilities;
use serde_json::json;

mod anthropic_capabilities_tests {
    use super::*;

    #[test]
    fn anthropic_supports_all_features() {
        let caps = ProviderCapabilities::anthropic();

        assert!(caps.supports_allof);
        assert!(caps.supports_anyof);
        assert!(caps.supports_oneof);
        assert!(caps.supports_if_then_else);
        assert!(caps.supports_ref);
        assert!(caps.supports_definitions);
        assert!(caps.supports_not);
        assert!(caps.supports_additional_properties);
        assert!(caps.supports_const);
    }

    #[test]
    fn anthropic_requires_no_transformation() {
        let caps = ProviderCapabilities::anthropic();

        let complex_schema = json!({
            "allOf": [{"type": "object"}],
            "anyOf": [{"type": "string"}],
            "oneOf": [{"type": "number"}],
            "if": {},
            "$ref": "#/definitions/Test",
            "definitions": {},
            "not": {}
        });

        assert!(!caps.requires_transformation(&complex_schema));
    }
}

mod openai_capabilities_tests {
    use super::*;

    #[test]
    fn openai_capabilities() {
        let caps = ProviderCapabilities::openai();

        assert!(caps.supports_allof);
        assert!(caps.supports_anyof);
        assert!(caps.supports_oneof);
        assert!(!caps.supports_if_then_else);
        assert!(caps.supports_ref);
        assert!(caps.supports_definitions);
        assert!(!caps.supports_not);
        assert!(caps.supports_additional_properties);
        assert!(caps.supports_const);
    }

    #[test]
    fn openai_requires_transformation_for_if_then() {
        let caps = ProviderCapabilities::openai();

        let schema_with_if = json!({
            "type": "object",
            "if": {
                "properties": {"type": {"const": "a"}}
            }
        });

        assert!(caps.requires_transformation(&schema_with_if));
    }

    #[test]
    fn openai_requires_transformation_for_not() {
        let caps = ProviderCapabilities::openai();

        let schema_with_not = json!({
            "type": "object",
            "not": {"type": "null"}
        });

        assert!(caps.requires_transformation(&schema_with_not));
    }

    #[test]
    fn openai_no_transformation_for_allof() {
        let caps = ProviderCapabilities::openai();

        let schema_with_allof = json!({
            "allOf": [{"type": "object"}]
        });

        assert!(!caps.requires_transformation(&schema_with_allof));
    }
}

mod gemini_capabilities_tests {
    use super::*;

    #[test]
    fn gemini_limited_capabilities() {
        let caps = ProviderCapabilities::gemini();

        assert!(!caps.supports_allof);
        assert!(caps.supports_anyof);
        assert!(!caps.supports_oneof);
        assert!(!caps.supports_if_then_else);
        assert!(caps.supports_ref);
        assert!(caps.supports_definitions);
        assert!(!caps.supports_not);
        assert!(!caps.supports_additional_properties);
        assert!(!caps.supports_const);
    }

    #[test]
    fn gemini_requires_transformation_for_allof() {
        let caps = ProviderCapabilities::gemini();

        let schema_with_allof = json!({
            "allOf": [
                {"type": "object"},
                {"properties": {"name": {"type": "string"}}}
            ]
        });

        assert!(caps.requires_transformation(&schema_with_allof));
    }

    #[test]
    fn gemini_requires_transformation_for_oneof() {
        let caps = ProviderCapabilities::gemini();

        let schema_with_oneof = json!({
            "oneOf": [
                {"type": "string"},
                {"type": "number"}
            ]
        });

        assert!(caps.requires_transformation(&schema_with_oneof));
    }

    #[test]
    fn gemini_requires_transformation_for_if_then() {
        let caps = ProviderCapabilities::gemini();

        let schema_with_if = json!({
            "if": {"properties": {"type": {"const": "a"}}},
            "then": {"required": ["data"]}
        });

        assert!(caps.requires_transformation(&schema_with_if));
    }

    #[test]
    fn gemini_no_transformation_for_anyof() {
        let caps = ProviderCapabilities::gemini();

        let schema_with_anyof = json!({
            "anyOf": [{"type": "string"}, {"type": "null"}]
        });

        assert!(!caps.requires_transformation(&schema_with_anyof));
    }
}

mod requires_transformation_tests {
    use super::*;

    #[test]
    fn simple_schema_no_transformation() {
        let caps = ProviderCapabilities::gemini();

        let simple_schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        });

        assert!(!caps.requires_transformation(&simple_schema));
    }

    #[test]
    fn null_value_no_transformation() {
        let caps = ProviderCapabilities::gemini();
        let schema = json!(null);
        assert!(!caps.requires_transformation(&schema));
    }

    #[test]
    fn array_value_no_transformation() {
        let caps = ProviderCapabilities::gemini();
        let schema = json!(["not", "an", "object"]);
        assert!(!caps.requires_transformation(&schema));
    }

    #[test]
    fn definitions_transformation_check() {
        let caps_with = ProviderCapabilities::anthropic();
        let caps_without = ProviderCapabilities {
            supports_definitions: false,
            ..ProviderCapabilities::anthropic()
        };

        let schema = json!({
            "definitions": {
                "Address": {"type": "object"}
            }
        });

        assert!(!caps_with.requires_transformation(&schema));
        assert!(caps_without.requires_transformation(&schema));
    }

    #[test]
    fn defs_transformation_check() {
        let caps_with = ProviderCapabilities::anthropic();
        let caps_without = ProviderCapabilities {
            supports_definitions: false,
            ..ProviderCapabilities::anthropic()
        };

        let schema = json!({
            "$defs": {
                "Person": {"type": "object"}
            }
        });

        assert!(!caps_with.requires_transformation(&schema));
        assert!(caps_without.requires_transformation(&schema));
    }

    #[test]
    fn ref_transformation_check() {
        let caps_with = ProviderCapabilities::anthropic();
        let caps_without = ProviderCapabilities {
            supports_ref: false,
            ..ProviderCapabilities::anthropic()
        };

        let schema = json!({
            "$ref": "#/definitions/SomeType"
        });

        assert!(!caps_with.requires_transformation(&schema));
        assert!(caps_without.requires_transformation(&schema));
    }
}

mod equality_tests {
    use super::*;

    #[test]
    fn capabilities_equality() {
        let caps1 = ProviderCapabilities::anthropic();
        let caps2 = ProviderCapabilities::anthropic();

        assert_eq!(caps1, caps2);
    }

    #[test]
    fn different_capabilities_not_equal() {
        let caps1 = ProviderCapabilities::anthropic();
        let caps2 = ProviderCapabilities::gemini();

        assert_ne!(caps1, caps2);
    }

    #[test]
    fn capabilities_is_copy() {
        let caps = ProviderCapabilities::openai();
        let copied = caps;
        assert_eq!(caps, copied);
    }
}
