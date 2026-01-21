//! Tests for SchemaSanitizer.

use systemprompt_ai::services::schema::{ProviderCapabilities, SchemaSanitizer};
use serde_json::json;

mod remove_unsupported_keywords_tests {
    use super::*;

    #[test]
    fn gemini_removes_allof() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let schema = json!({
            "type": "object",
            "allOf": [{"type": "object"}]
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("allOf").is_none());
        assert!(result.get("type").is_some());
    }

    #[test]
    fn gemini_removes_oneof() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let schema = json!({
            "oneOf": [{"type": "string"}, {"type": "number"}]
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("oneOf").is_none());
    }

    #[test]
    fn gemini_keeps_anyof() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let schema = json!({
            "anyOf": [{"type": "string"}, {"type": "null"}]
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("anyOf").is_some());
    }

    #[test]
    fn openai_removes_if_then_else() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::openai());
        let schema = json!({
            "if": {"properties": {"type": {"const": "a"}}},
            "then": {"required": ["data"]},
            "else": {"required": ["other"]}
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("if").is_none());
        assert!(result.get("then").is_none());
        assert!(result.get("else").is_none());
    }

    #[test]
    fn openai_removes_not() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::openai());
        let schema = json!({
            "type": "string",
            "not": {"pattern": "^forbidden"}
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("not").is_none());
    }

    #[test]
    fn gemini_removes_additional_properties() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let schema = json!({
            "type": "object",
            "additionalProperties": false
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("additionalProperties").is_none());
    }

    #[test]
    fn anthropic_keeps_all_keywords() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::anthropic());
        let schema = json!({
            "allOf": [{"type": "object"}],
            "anyOf": [{"type": "string"}],
            "oneOf": [{"type": "number"}],
            "if": {},
            "then": {},
            "not": {},
            "additionalProperties": true
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("allOf").is_some());
        assert!(result.get("anyOf").is_some());
        assert!(result.get("oneOf").is_some());
        assert!(result.get("if").is_some());
        assert!(result.get("then").is_some());
        assert!(result.get("not").is_some());
        assert!(result.get("additionalProperties").is_some());
    }
}

mod remove_metadata_fields_tests {
    use super::*;

    fn sanitizer() -> SchemaSanitizer {
        SchemaSanitizer::new(ProviderCapabilities::anthropic())
    }

    #[test]
    fn removes_schema_field() {
        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object"
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("$schema").is_none());
        assert!(result.get("type").is_some());
    }

    #[test]
    fn removes_id_field() {
        let schema = json!({
            "$id": "https://example.com/schema",
            "type": "string"
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("$id").is_none());
    }

    #[test]
    fn removes_readonly_field() {
        let schema = json!({
            "type": "string",
            "readOnly": true
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("readOnly").is_none());
    }

    #[test]
    fn removes_writeonly_field() {
        let schema = json!({
            "type": "string",
            "writeOnly": true
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("writeOnly").is_none());
    }

    #[test]
    fn removes_deprecated_field() {
        let schema = json!({
            "type": "string",
            "deprecated": true
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("deprecated").is_none());
    }

    #[test]
    fn removes_examples_field() {
        let schema = json!({
            "type": "string",
            "examples": ["example1", "example2"]
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("examples").is_none());
    }

    #[test]
    fn removes_content_media_type() {
        let schema = json!({
            "type": "string",
            "contentMediaType": "application/json"
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("contentMediaType").is_none());
    }

    #[test]
    fn removes_content_encoding() {
        let schema = json!({
            "type": "string",
            "contentEncoding": "base64"
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("contentEncoding").is_none());
    }

    #[test]
    fn removes_output_schema() {
        let schema = json!({
            "type": "object",
            "outputSchema": {"type": "string"}
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("outputSchema").is_none());
    }
}

mod remove_extension_fields_tests {
    use super::*;

    fn sanitizer() -> SchemaSanitizer {
        SchemaSanitizer::new(ProviderCapabilities::anthropic())
    }

    #[test]
    fn removes_x_prefixed_fields() {
        let schema = json!({
            "type": "object",
            "x-custom": "value",
            "x-another": 123,
            "x-complex": {"nested": true}
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("x-custom").is_none());
        assert!(result.get("x-another").is_none());
        assert!(result.get("x-complex").is_none());
    }

    #[test]
    fn keeps_non_extension_fields() {
        let schema = json!({
            "type": "object",
            "xProperty": "not an extension",
            "properties": {}
        });

        let result = sanitizer().sanitize(schema);
        assert!(result.get("xProperty").is_some());
        assert!(result.get("properties").is_some());
    }
}

mod convert_const_to_enum_tests {
    use super::*;

    #[test]
    fn gemini_converts_const_to_enum() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let schema = json!({
            "type": "string",
            "const": "fixed_value"
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("const").is_none());
        assert!(result.get("enum").is_some());

        let enum_values = result.get("enum").unwrap().as_array().unwrap();
        assert_eq!(enum_values.len(), 1);
        assert_eq!(enum_values[0], "fixed_value");
    }

    #[test]
    fn anthropic_keeps_const() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::anthropic());
        let schema = json!({
            "type": "string",
            "const": "fixed_value"
        });

        let result = sanitizer.sanitize(schema);
        assert!(result.get("const").is_some());
        assert!(result.get("enum").is_none());
    }

    #[test]
    fn gemini_converts_numeric_const() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());
        let schema = json!({
            "type": "integer",
            "const": 42
        });

        let result = sanitizer.sanitize(schema);
        let enum_values = result.get("enum").unwrap().as_array().unwrap();
        assert_eq!(enum_values[0], 42);
    }
}

mod sanitize_nested_schemas_tests {
    use super::*;

    fn sanitizer() -> SchemaSanitizer {
        SchemaSanitizer::new(ProviderCapabilities::gemini())
    }

    #[test]
    fn sanitizes_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "allOf": [{"type": "object"}],
                    "$schema": "remove-me"
                }
            }
        });

        let result = sanitizer().sanitize(schema);
        let nested = result["properties"]["nested"].as_object().unwrap();
        assert!(nested.get("allOf").is_none());
        assert!(nested.get("$schema").is_none());
        assert!(nested.get("type").is_some());
    }

    #[test]
    fn sanitizes_items() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "object",
                "allOf": [{"type": "string"}],
                "x-custom": "remove"
            }
        });

        let result = sanitizer().sanitize(schema);
        let items = result["items"].as_object().unwrap();
        assert!(items.get("allOf").is_none());
        assert!(items.get("x-custom").is_none());
    }

    #[test]
    fn sanitizes_anyof_items() {
        let schema = json!({
            "anyOf": [
                {
                    "type": "string",
                    "$schema": "remove"
                },
                {
                    "type": "number",
                    "x-custom": "remove"
                }
            ]
        });

        let result = sanitizer().sanitize(schema);
        let any_of = result["anyOf"].as_array().unwrap();
        assert!(any_of[0].get("$schema").is_none());
        assert!(any_of[1].get("x-custom").is_none());
    }

    #[test]
    fn sanitizes_additional_properties_schema() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::anthropic());
        let schema = json!({
            "type": "object",
            "additionalProperties": {
                "type": "string",
                "$schema": "remove",
                "x-custom": "remove"
            }
        });

        let result = sanitizer.sanitize(schema);
        let additional = result["additionalProperties"].as_object().unwrap();
        assert!(additional.get("$schema").is_none());
        assert!(additional.get("x-custom").is_none());
        assert!(additional.get("type").is_some());
    }

    #[test]
    fn deeply_nested_sanitization() {
        let schema = json!({
            "type": "object",
            "properties": {
                "level1": {
                    "type": "object",
                    "properties": {
                        "level2": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "x-deep": "remove",
                                "$schema": "remove"
                            }
                        }
                    }
                }
            }
        });

        let result = sanitizer().sanitize(schema);
        let deep = &result["properties"]["level1"]["properties"]["level2"]["items"];
        assert!(deep.get("x-deep").is_none());
        assert!(deep.get("$schema").is_none());
    }
}

mod non_object_handling_tests {
    use super::*;

    #[test]
    fn returns_non_object_unchanged() {
        let sanitizer = SchemaSanitizer::new(ProviderCapabilities::gemini());

        let string_schema = json!("just a string");
        assert_eq!(sanitizer.sanitize(string_schema.clone()), string_schema);

        let array_schema = json!(["array", "values"]);
        assert_eq!(sanitizer.sanitize(array_schema.clone()), array_schema);

        let null_schema = json!(null);
        assert_eq!(sanitizer.sanitize(null_schema.clone()), null_schema);

        let number_schema = json!(42);
        assert_eq!(sanitizer.sanitize(number_schema.clone()), number_schema);
    }
}
