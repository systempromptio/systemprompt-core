use serde_json::json;
use systemprompt_ai::services::structured_output::StructuredOutputProcessor;
use systemprompt_ai::{ResponseFormat, StructuredOutputOptions};

mod process_response_tests {
    use super::*;

    #[test]
    fn parses_valid_json_with_text_format() {
        let content = r#"{"key": "value"}"#;
        let format = ResponseFormat::Text;
        let options = StructuredOutputOptions::new();

        let result = StructuredOutputProcessor::process_response(content, &format, &options);
        let value = result.unwrap();
        assert_eq!(value["key"], "value");
    }

    #[test]
    fn parses_json_with_json_object_format() {
        let content = r#"{"status": "ok", "count": 42}"#;
        let format = ResponseFormat::json_object();
        let options = StructuredOutputOptions::new();

        let result =
            StructuredOutputProcessor::process_response(content, &format, &options).unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["count"], 42);
    }

    #[test]
    fn validates_against_json_schema_strict() {
        let content = r#"{"name": "Alice", "age": 30}"#;
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name", "age"]
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: Some("person".to_string()),
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            validate_schema: Some(true),
            ..Default::default()
        };

        let result =
            StructuredOutputProcessor::process_response(content, &format, &options).unwrap();
        assert_eq!(result["name"], "Alice");
    }

    #[test]
    fn rejects_invalid_schema_match() {
        let content = r#"{"name": 123}"#;
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            validate_schema: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::process_response(content, &format, &options);
        assert!(result.is_err());
    }

    #[test]
    fn skips_validation_when_disabled() {
        let content = r#"{"name": 123}"#;
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            validate_schema: Some(false),
            ..Default::default()
        };

        let result =
            StructuredOutputProcessor::process_response(content, &format, &options).unwrap();
        assert_eq!(result["name"], 123);
    }

    #[test]
    fn extracts_json_from_markdown_before_validation() {
        let content = "Here is the result:\n```json\n{\"value\": 42}\n```\nDone.";
        let schema = json!({
            "type": "object",
            "properties": {
                "value": {"type": "integer"}
            }
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            validate_schema: Some(true),
            ..Default::default()
        };

        let result =
            StructuredOutputProcessor::process_response(content, &format, &options).unwrap();
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn uses_custom_extraction_pattern() {
        let content = "RESPONSE_START{\"data\": true}RESPONSE_END";
        let format = ResponseFormat::Text;
        let options = StructuredOutputOptions {
            extraction_pattern: Some(r"RESPONSE_START([\s\S]*?)RESPONSE_END".to_string()),
            ..Default::default()
        };

        let result =
            StructuredOutputProcessor::process_response(content, &format, &options).unwrap();
        assert_eq!(result["data"], true);
    }

    #[test]
    fn returns_error_for_no_json() {
        let content = "This is plain text with no JSON";
        let format = ResponseFormat::Text;
        let options = StructuredOutputOptions::new();

        let result = StructuredOutputProcessor::process_response(content, &format, &options);
        assert!(result.is_err());
    }

    #[test]
    fn validates_missing_required_field() {
        let content = r#"{"name": "Alice"}"#;
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name", "age"]
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            validate_schema: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::process_response(content, &format, &options);
        assert!(result.is_err());
    }

    #[test]
    fn defaults_to_strict_validation() {
        let content = r#"{"name": "Alice"}"#;
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name", "age"]
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: None,
        };
        let options = StructuredOutputOptions {
            validate_schema: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::process_response(content, &format, &options);
        assert!(result.is_err());
    }

    #[test]
    fn defaults_validate_schema_to_true() {
        let content = r#"{"name": 123}"#;
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: Some(true),
        };
        let options = StructuredOutputOptions::new();

        let result = StructuredOutputProcessor::process_response(content, &format, &options);
        assert!(result.is_err());
    }
}

mod enhance_prompt_for_json_tests {
    use super::*;

    #[test]
    fn text_format_returns_original() {
        let prompt = "Tell me about cats";
        let format = ResponseFormat::Text;
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert_eq!(result, prompt);
    }

    #[test]
    fn json_object_adds_instruction() {
        let prompt = "Give me user info";
        let format = ResponseFormat::json_object();
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert!(result.contains("Give me user info"));
        assert!(result.contains("valid JSON"));
    }

    #[test]
    fn json_schema_includes_schema() {
        let prompt = "Generate data";
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        let format = ResponseFormat::JsonSchema {
            schema: schema.clone(),
            name: Some("test_schema".to_string()),
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert!(result.contains("Generate data"));
        assert!(result.contains("test_schema"));
        assert!(result.contains("string"));
    }

    #[test]
    fn json_schema_uses_default_name() {
        let prompt = "Generate data";
        let schema = json!({"type": "object"});
        let format = ResponseFormat::JsonSchema {
            schema,
            name: None,
            strict: Some(true),
        };
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert!(result.contains("response"));
    }

    #[test]
    fn skips_injection_when_disabled() {
        let prompt = "Do something";
        let format = ResponseFormat::json_object();
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(false),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert_eq!(result, prompt);
    }

    #[test]
    fn defaults_inject_to_true() {
        let prompt = "Do something";
        let format = ResponseFormat::json_object();
        let options = StructuredOutputOptions::new();

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert!(result.contains("valid JSON"));
    }

    #[test]
    fn empty_prompt_still_gets_injection() {
        let prompt = "";
        let format = ResponseFormat::json_object();
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert!(result.contains("valid JSON"));
    }

    #[test]
    fn preserves_multiline_prompt() {
        let prompt = "Line 1\nLine 2\nLine 3";
        let format = ResponseFormat::json_object();
        let options = StructuredOutputOptions {
            inject_json_prompt: Some(true),
            ..Default::default()
        };

        let result = StructuredOutputProcessor::enhance_prompt_for_json(prompt, &format, &options);
        assert!(result.contains("Line 1\nLine 2\nLine 3"));
    }
}
