//! Tests for ProviderCapabilities.

use systemprompt_ai::services::schema::ProviderCapabilities;
use systemprompt_ai::services::schema::capabilities::SchemaFeatures;
use serde_json::json;

mod anthropic_capabilities_tests {
    use super::*;

    #[test]
    fn anthropic_supports_all_features() {
        let caps = ProviderCapabilities::anthropic();

        assert!(caps.composition.allof);
        assert!(caps.composition.anyof);
        assert!(caps.composition.oneof);
        assert!(caps.composition.if_then_else);
        assert!(caps.features.references);
        assert!(caps.features.definitions);
        assert!(caps.composition.not);
        assert!(caps.features.additional_properties);
        assert!(caps.features.const_values);
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

        assert!(caps.composition.allof);
        assert!(caps.composition.anyof);
        assert!(caps.composition.oneof);
        assert!(!caps.composition.if_then_else);
        assert!(caps.features.references);
        assert!(caps.features.definitions);
        assert!(!caps.composition.not);
        assert!(caps.features.additional_properties);
        assert!(caps.features.const_values);
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

        assert!(!caps.composition.allof);
        assert!(caps.composition.anyof);
        assert!(!caps.composition.oneof);
        assert!(!caps.composition.if_then_else);
        assert!(!caps.features.references);
        assert!(!caps.features.definitions);
        assert!(!caps.composition.not);
        assert!(!caps.features.additional_properties);
        assert!(!caps.features.const_values);
    }

    #[test]
    fn gemini_requires_transformation_for_ref() {
        let caps = ProviderCapabilities::gemini();
        let schema = json!({"$ref": "#/$defs/Foo"});
        assert!(caps.requires_transformation(&schema));
    }

    #[test]
    fn gemini_requires_transformation_for_defs() {
        let caps = ProviderCapabilities::gemini();
        let schema = json!({"$defs": {"Foo": {"type": "string"}}});
        assert!(caps.requires_transformation(&schema));
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
        let anthropic = ProviderCapabilities::anthropic();
        let caps_without = ProviderCapabilities {
            composition: anthropic.composition,
            features: SchemaFeatures {
                definitions: false,
                ..anthropic.features
            },
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
        let anthropic = ProviderCapabilities::anthropic();
        let caps_without = ProviderCapabilities {
            composition: anthropic.composition,
            features: SchemaFeatures {
                definitions: false,
                ..anthropic.features
            },
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
        let anthropic = ProviderCapabilities::anthropic();
        let caps_without = ProviderCapabilities {
            composition: anthropic.composition,
            features: SchemaFeatures {
                references: false,
                ..anthropic.features
            },
        };

        let schema = json!({
            "$ref": "#/definitions/SomeType"
        });

        assert!(!caps_with.requires_transformation(&schema));
        assert!(caps_without.requires_transformation(&schema));
    }
}
