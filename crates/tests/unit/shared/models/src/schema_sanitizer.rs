use serde_json::{Value, json};
use systemprompt_models::schema::{
    ProviderCapabilities, SchemaComposition, SchemaFeatures, SchemaSanitizer,
};

fn all_disabled() -> ProviderCapabilities {
    ProviderCapabilities {
        composition: SchemaComposition {
            allof: false,
            anyof: false,
            oneof: false,
            if_then_else: false,
            not: false,
        },
        features: SchemaFeatures {
            references: false,
            definitions: false,
            additional_properties: false,
            const_values: false,
            exclusive_bounds: false,
            property_names: false,
        },
    }
}

fn all_enabled() -> ProviderCapabilities {
    ProviderCapabilities {
        composition: SchemaComposition {
            allof: true,
            anyof: true,
            oneof: true,
            if_then_else: true,
            not: true,
        },
        features: SchemaFeatures {
            references: true,
            definitions: true,
            additional_properties: true,
            const_values: true,
            exclusive_bounds: true,
            property_names: true,
        },
    }
}

mod capabilities_matrices {
    use super::*;

    #[test]
    fn anthropic_supports_everything() {
        let caps = ProviderCapabilities::anthropic();
        assert!(caps.composition.allof);
        assert!(caps.composition.anyof);
        assert!(caps.composition.oneof);
        assert!(caps.composition.if_then_else);
        assert!(caps.composition.not);
        assert!(caps.features.references);
        assert!(caps.features.definitions);
        assert!(caps.features.additional_properties);
        assert!(caps.features.const_values);
        assert!(caps.features.exclusive_bounds);
        assert!(caps.features.property_names);
    }

    #[test]
    fn openai_drops_if_then_else_and_not() {
        let caps = ProviderCapabilities::openai();
        assert!(!caps.composition.if_then_else);
        assert!(!caps.composition.not);
        assert!(caps.composition.allof);
        assert!(caps.composition.anyof);
        assert!(caps.composition.oneof);
        assert!(caps.features.references);
        assert!(caps.features.const_values);
    }

    #[test]
    fn gemini_is_the_most_restrictive() {
        let caps = ProviderCapabilities::gemini();
        assert!(!caps.composition.allof);
        assert!(caps.composition.anyof);
        assert!(!caps.composition.oneof);
        assert!(!caps.composition.if_then_else);
        assert!(!caps.composition.not);
        assert!(!caps.features.references);
        assert!(!caps.features.definitions);
        assert!(!caps.features.additional_properties);
        assert!(!caps.features.const_values);
        assert!(!caps.features.exclusive_bounds);
        assert!(!caps.features.property_names);
    }

    #[test]
    fn caps_are_copy_and_eq() {
        let a = ProviderCapabilities::anthropic();
        let b = a;
        assert_eq!(a, b);
        assert_ne!(
            ProviderCapabilities::anthropic(),
            ProviderCapabilities::gemini()
        );
    }
}

mod requires_transformation {
    use super::*;

    #[test]
    fn flags_allof_when_unsupported() {
        let caps = ProviderCapabilities::gemini();
        assert!(caps.requires_transformation(&json!({"allOf": []})));
    }

    #[test]
    fn flags_oneof_when_unsupported() {
        let caps = ProviderCapabilities::gemini();
        assert!(caps.requires_transformation(&json!({"oneOf": []})));
    }

    #[test]
    fn flags_if_when_unsupported() {
        let caps = ProviderCapabilities::openai();
        assert!(caps.requires_transformation(&json!({"if": {}})));
    }

    #[test]
    fn flags_not_when_unsupported() {
        let caps = ProviderCapabilities::openai();
        assert!(caps.requires_transformation(&json!({"not": {}})));
    }

    #[test]
    fn flags_ref_when_unsupported() {
        let caps = ProviderCapabilities::gemini();
        assert!(caps.requires_transformation(&json!({"$ref": "#/x"})));
    }

    #[test]
    fn flags_definitions_and_defs_when_unsupported() {
        let caps = ProviderCapabilities::gemini();
        assert!(caps.requires_transformation(&json!({"definitions": {}})));
        assert!(caps.requires_transformation(&json!({"$defs": {}})));
    }

    #[test]
    fn anyof_supported_by_gemini_is_not_flagged() {
        let caps = ProviderCapabilities::gemini();
        assert!(!caps.requires_transformation(&json!({"anyOf": []})));
    }

    #[test]
    fn anthropic_never_requires_transformation() {
        let caps = ProviderCapabilities::anthropic();
        assert!(!caps.requires_transformation(&json!({
            "allOf": [], "anyOf": [], "oneOf": [], "if": {}, "not": {},
            "$ref": "#/x", "definitions": {}, "$defs": {}
        })));
    }

    #[test]
    fn non_object_schema_is_never_flagged() {
        let caps = ProviderCapabilities::gemini();
        assert!(!caps.requires_transformation(&json!("a string")));
        assert!(!caps.requires_transformation(&json!(42)));
        assert!(!caps.requires_transformation(&Value::Null));
    }
}

mod sanitize_nullable_normalisation {
    use super::*;

    #[test]
    fn type_array_with_null_becomes_nullable_flag() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({"type": ["string", "null"]}));
        assert_eq!(out["type"], json!("string"));
        assert_eq!(out["nullable"], json!(true));
    }

    #[test]
    fn type_array_all_null_drops_type() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({"type": ["null"]}));
        assert!(out.get("type").is_none());
        assert_eq!(out["nullable"], json!(true));
    }

    #[test]
    fn type_array_multiple_non_null_kept_as_array() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({"type": ["string", "integer", "null"]}));
        assert_eq!(out["type"], json!(["string", "integer"]));
        assert_eq!(out["nullable"], json!(true));
    }

    #[test]
    fn type_array_without_null_left_untouched() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({"type": ["string", "integer"]}));
        assert_eq!(out["type"], json!(["string", "integer"]));
        assert!(out.get("nullable").is_none());
    }

    #[test]
    fn anyof_with_single_null_collapses_and_inlines() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "anyOf": [{"type": "string"}, {"type": "null"}]
        }));
        assert!(out.get("anyOf").is_none());
        assert_eq!(out["type"], json!("string"));
        assert_eq!(out["nullable"], json!(true));
    }

    #[test]
    fn anyof_with_null_and_multiple_keeps_array() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "anyOf": [{"type": "string"}, {"type": "integer"}, {"type": "null"}]
        }));
        let arr = out["anyOf"].as_array().expect("anyOf array");
        assert_eq!(arr.len(), 2);
        assert_eq!(out["nullable"], json!(true));
    }
}

mod sanitize_keyword_removal {
    use super::*;

    #[test]
    fn removes_all_composition_keywords_when_disabled() {
        let s = SchemaSanitizer::new(all_disabled());
        let out = s.sanitize(json!({
            "allOf": [{}], "anyOf": [{}], "oneOf": [{}],
            "if": {}, "then": {}, "else": {}, "not": {}
        }));
        let obj = out.as_object().expect("object");
        for k in ["allOf", "anyOf", "oneOf", "if", "then", "else", "not"] {
            assert!(!obj.contains_key(k), "{k} should be removed");
        }
    }

    #[test]
    fn removes_refs_and_definitions_when_disabled() {
        let s = SchemaSanitizer::new(all_disabled());
        let out = s.sanitize(json!({
            "$ref": "#/a", "definitions": {"x": {}}, "$defs": {"y": {}}
        }));
        let obj = out.as_object().expect("object");
        assert!(!obj.contains_key("$ref"));
        assert!(!obj.contains_key("definitions"));
        assert!(!obj.contains_key("$defs"));
    }

    #[test]
    fn removes_additional_properties_and_bounds_and_property_names() {
        let s = SchemaSanitizer::new(all_disabled());
        let out = s.sanitize(json!({
            "type": "object",
            "additionalProperties": false,
            "exclusiveMinimum": 0,
            "exclusiveMaximum": 10,
            "propertyNames": {"pattern": "^a"},
            "patternProperties": {"^x": {}}
        }));
        let obj = out.as_object().expect("object");
        assert!(!obj.contains_key("additionalProperties"));
        assert!(!obj.contains_key("exclusiveMinimum"));
        assert!(!obj.contains_key("exclusiveMaximum"));
        assert!(!obj.contains_key("propertyNames"));
        assert!(!obj.contains_key("patternProperties"));
    }

    #[test]
    fn keeps_supported_keywords_when_enabled() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "type": "object",
            "additionalProperties": false,
            "exclusiveMinimum": 0
        }));
        assert_eq!(out["additionalProperties"], json!(false));
        assert_eq!(out["exclusiveMinimum"], json!(0));
    }
}

mod sanitize_metadata_and_extensions {
    use super::*;

    #[test]
    fn strips_metadata_fields() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "type": "object",
            "$schema": "x", "$id": "y", "readOnly": true, "writeOnly": true,
            "deprecated": true, "examples": [1], "contentMediaType": "text/plain",
            "contentEncoding": "base64", "outputSchema": {}
        }));
        let obj = out.as_object().expect("object");
        for k in [
            "$schema",
            "$id",
            "readOnly",
            "writeOnly",
            "deprecated",
            "examples",
            "contentMediaType",
            "contentEncoding",
            "outputSchema",
        ] {
            assert!(!obj.contains_key(k), "{k} should be stripped");
        }
        assert_eq!(out["type"], json!("object"));
    }

    #[test]
    fn strips_x_prefixed_extension_fields() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "type": "string", "x-internal": true, "x-foo": "bar", "keep": 1
        }));
        let obj = out.as_object().expect("object");
        assert!(!obj.contains_key("x-internal"));
        assert!(!obj.contains_key("x-foo"));
        assert_eq!(out["keep"], json!(1));
    }
}

mod sanitize_const_conversion {
    use super::*;

    #[test]
    fn const_becomes_single_value_enum_when_unsupported() {
        let s = SchemaSanitizer::new(all_disabled());
        let out = s.sanitize(json!({"const": "fixed"}));
        assert!(out.get("const").is_none());
        assert_eq!(out["enum"], json!(["fixed"]));
    }

    #[test]
    fn const_kept_when_supported() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({"const": "fixed"}));
        assert_eq!(out["const"], json!("fixed"));
        assert!(out.get("enum").is_none());
    }
}

mod sanitize_recursion {
    use super::*;

    #[test]
    fn recurses_into_properties() {
        let s = SchemaSanitizer::new(all_disabled());
        let out = s.sanitize(json!({
            "type": "object",
            "properties": {
                "a": {"type": "string", "x-secret": true},
                "b": {"const": "v"}
            }
        }));
        assert!(out["properties"]["a"].get("x-secret").is_none());
        assert_eq!(out["properties"]["b"]["enum"], json!(["v"]));
    }

    #[test]
    fn recurses_into_items() {
        let s = SchemaSanitizer::new(all_disabled());
        let out = s.sanitize(json!({
            "type": "array",
            "items": {"type": "string", "x-meta": 1}
        }));
        assert!(out["items"].get("x-meta").is_none());
    }

    #[test]
    fn recurses_into_object_additional_properties() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "type": "object",
            "additionalProperties": {"type": "string", "x-meta": 1}
        }));
        assert!(out["additionalProperties"].get("x-meta").is_none());
        assert_eq!(out["additionalProperties"]["type"], json!("string"));
    }

    #[test]
    fn recurses_into_anyof_branches_when_supported() {
        let s = SchemaSanitizer::new(all_enabled());
        let out = s.sanitize(json!({
            "anyOf": [
                {"type": "string", "x-a": 1},
                {"type": "integer", "x-b": 2}
            ]
        }));
        let arr = out["anyOf"].as_array().expect("anyOf array");
        assert_eq!(arr.len(), 2);
        assert!(arr[0].get("x-a").is_none());
        assert!(arr[1].get("x-b").is_none());
    }
}

mod sanitize_edge_cases {
    use super::*;

    #[test]
    fn non_object_value_returned_unchanged() {
        let s = SchemaSanitizer::new(all_enabled());
        assert_eq!(s.sanitize(json!("scalar")), json!("scalar"));
        assert_eq!(s.sanitize(json!(7)), json!(7));
        assert_eq!(s.sanitize(json!([1, 2])), json!([1, 2]));
    }

    #[test]
    fn empty_object_returned_as_empty_object() {
        let s = SchemaSanitizer::new(all_enabled());
        assert_eq!(s.sanitize(json!({})), json!({}));
    }

    #[test]
    fn sanitizer_is_copy() {
        let s = SchemaSanitizer::new(all_enabled());
        let t = s;
        assert_eq!(t.sanitize(json!({})), json!({}));
    }

    #[test]
    fn idempotent_on_already_clean_schema() {
        let s = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let clean = json!({
            "type": "object",
            "properties": {"q": {"type": "string"}}
        });
        let once = s.sanitize(clean.clone());
        let twice = s.sanitize(once.clone());
        assert_eq!(once, twice);
    }
}
