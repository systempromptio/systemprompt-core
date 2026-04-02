use systemprompt_ai::services::structured_output::parser::JsonParser;

mod extract_json_edge_cases {
    use super::*;

    #[test]
    fn extracts_json_after_long_preamble() {
        let content = format!("{} {}", "word ".repeat(100), r#"{"found": true}"#);
        let result = JsonParser::extract_json(&content, None).unwrap();
        assert_eq!(result["found"], true);
    }

    #[test]
    fn handles_json_with_newlines_in_strings() {
        let content = r#"{"text": "line1\nline2\nline3"}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert!(result["text"].is_string());
    }

    #[test]
    fn handles_deeply_nested_json() {
        let content = r#"{"a":{"b":{"c":{"d":{"e":"deep"}}}}}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["a"]["b"]["c"]["d"]["e"], "deep");
    }

    #[test]
    fn handles_json_with_numeric_string_values() {
        let content = r#"{"port": "8080", "code": "200"}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["port"], "8080");
    }

    #[test]
    fn handles_json_with_boolean_values() {
        let content = r#"{"enabled": true, "debug": false}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["enabled"], true);
        assert_eq!(result["debug"], false);
    }

    #[test]
    fn handles_whitespace_only_content() {
        let content = "   \n\t\n   ";
        let result = JsonParser::extract_json(content, None);
        assert!(result.is_err());
    }

    #[test]
    fn handles_json_array_in_markdown() {
        let content = "Results:\n```json\n[1, 2, 3]\n```";
        let result = JsonParser::extract_json(content, None).unwrap();
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 3);
    }

    #[test]
    fn handles_json_with_null_values() {
        let content = r#"{"name": null, "value": null}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert!(result["name"].is_null());
        assert!(result["value"].is_null());
    }

    #[test]
    fn handles_json_with_empty_string_values() {
        let content = r#"{"key": "", "other": ""}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["key"], "");
    }

    #[test]
    fn extracts_json_from_multiple_code_blocks() {
        let content = "First block:\n```json\n{\"a\": 1}\n```\nSecond:\n```json\n{\"b\": 2}\n```";
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["a"], 1);
    }

    #[test]
    fn handles_json_with_large_numbers() {
        let content = r#"{"big": 9999999999999, "float": 1.23456789012345}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["big"], 9_999_999_999_999_i64);
    }

    #[test]
    fn custom_pattern_invalid_regex_falls_back() {
        let content = r#"{"key": "value"}"#;
        let pattern = r"[invalid(regex";
        let result = JsonParser::extract_json(content, Some(pattern));
        assert!(result.is_ok());
    }
}

mod clean_json_string_edge_cases {
    use super::*;

    #[test]
    fn handles_empty_string() {
        let result = JsonParser::clean_json_string("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn removes_line_comments() {
        let input = r#"{"key": "value"} // this is a comment"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(!cleaned.contains("//"));
        assert!(!cleaned.contains("comment"));
    }

    #[test]
    fn handles_nested_trailing_commas() {
        let input = r#"{"a": {"b": 1,}, "c": [1, 2,],}"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(!cleaned.contains(",}"));
        assert!(!cleaned.contains(",]"));
    }

    #[test]
    fn preserves_valid_json() {
        let input = r#"{"valid": true, "array": [1, 2, 3]}"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["valid"], true);
    }

    #[test]
    fn handles_multiline_block_comments() {
        let input = "{\n  /* multi\n     line\n     comment */\n  \"key\": 1\n}";
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(!cleaned.contains("/*"));
        assert!(!cleaned.contains("*/"));
        assert!(!cleaned.contains("multi"));
    }

    #[test]
    fn handles_multiple_single_quote_keys() {
        let input = "{'a': 1, 'b': 2, 'c': 3}";
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(cleaned.contains("\"a\":"));
        assert!(cleaned.contains("\"b\":"));
        assert!(cleaned.contains("\"c\":"));
    }

    #[test]
    fn preserves_double_quoted_strings() {
        let input = r#"{"key": "value"}"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert_eq!(cleaned, input);
    }
}
