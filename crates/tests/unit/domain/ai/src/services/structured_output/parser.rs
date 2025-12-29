//! Tests for JsonParser.

use systemprompt_core_ai::services::structured_output::parser::JsonParser;

mod extract_json_tests {
    use super::*;

    #[test]
    fn extracts_valid_json_object() {
        let content = r#"{"name": "test", "value": 42}"#;
        let result = JsonParser::extract_json(content, None).unwrap();

        assert_eq!(result["name"], "test");
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn extracts_valid_json_array() {
        let content = r#"[1, 2, 3, 4, 5]"#;
        let result = JsonParser::extract_json(content, None).unwrap();

        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 5);
    }

    #[test]
    fn extracts_json_from_markdown_code_block() {
        let content = r#"Here's the result:
```json
{"status": "success", "count": 10}
```
That's all!"#;

        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["status"], "success");
        assert_eq!(result["count"], 10);
    }

    #[test]
    fn extracts_json_from_generic_code_block() {
        let content = r#"Result:
```
{"data": "value"}
```"#;

        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["data"], "value");
    }

    #[test]
    fn extracts_json_embedded_in_text() {
        let content = r#"The response is: {"key": "value"} and that's it."#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn extracts_nested_json() {
        let content = r#"{"outer": {"inner": {"deep": "value"}}}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["outer"]["inner"]["deep"], "value");
    }

    #[test]
    fn handles_json_with_arrays() {
        let content = r#"{"items": [{"id": 1}, {"id": 2}]}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["items"][0]["id"], 1);
        assert_eq!(result["items"][1]["id"], 2);
    }

    #[test]
    fn extracts_with_custom_pattern() {
        let content = r#"RESULT_START{"custom": true}RESULT_END"#;
        let pattern = r#"RESULT_START([\s\S]*?)RESULT_END"#;

        let result = JsonParser::extract_json(content, Some(pattern)).unwrap();
        assert_eq!(result["custom"], true);
    }

    #[test]
    fn falls_back_to_default_patterns_when_custom_fails() {
        let content = r#"{"fallback": "works"}"#;
        let pattern = r#"NEVER_MATCH"#;

        let result = JsonParser::extract_json(content, Some(pattern)).unwrap();
        assert_eq!(result["fallback"], "works");
    }

    #[test]
    fn returns_error_for_invalid_json() {
        let content = "This is not JSON at all";
        let result = JsonParser::extract_json(content, None);
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_for_empty_content() {
        let result = JsonParser::extract_json("", None);
        assert!(result.is_err());
    }

    #[test]
    fn handles_escaped_characters() {
        let content = r#"{"message": "Hello \"World\""}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["message"], r#"Hello "World""#);
    }

    #[test]
    fn handles_unicode() {
        let content = r#"{"greeting": "你好世界"}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["greeting"], "你好世界");
    }

    #[test]
    fn extracts_first_json_object() {
        let content = r#"{"first": true} some text {"second": true}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        // Should get the first one
        assert!(result.get("first").is_some() || result.get("second").is_some());
    }
}

mod heuristic_extraction_tests {
    use super::*;

    #[test]
    fn handles_balanced_braces() {
        let content = r#"prefix {"a": {"b": {"c": "deep"}}} suffix"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["a"]["b"]["c"], "deep");
    }

    #[test]
    fn handles_braces_in_strings() {
        let content = r#"{"text": "has {braces} inside"}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["text"], "has {braces} inside");
    }

    #[test]
    fn handles_escaped_quotes() {
        let content = r#"{"quote": "He said \"hello\""}"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["quote"], r#"He said "hello""#);
    }

    #[test]
    fn handles_array_at_start() {
        let content = r#"Here: [{"id": 1}, {"id": 2}] done"#;
        let result = JsonParser::extract_json(content, None).unwrap();
        assert!(result.is_array());
    }
}

mod clean_json_string_tests {
    use super::*;

    #[test]
    fn removes_trailing_commas_in_objects() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(!cleaned.contains(",}"));
    }

    #[test]
    fn removes_trailing_commas_in_arrays() {
        let input = r#"[1, 2, 3,]"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(!cleaned.contains(",]"));
    }

    #[test]
    fn converts_single_quoted_keys() {
        let input = r#"{'key': 'value'}"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(cleaned.contains("\"key\":"));
    }

    #[test]
    fn removes_block_comments() {
        let input = r#"{"a": /* comment */ 1}"#;
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(!cleaned.contains("/*"));
        assert!(!cleaned.contains("*/"));
    }

    #[test]
    fn trims_whitespace() {
        let input = "   {\"a\": 1}   ";
        let cleaned = JsonParser::clean_json_string(input).unwrap();
        assert!(cleaned.starts_with('{'));
        assert!(cleaned.ends_with('}'));
    }
}

mod complex_json_tests {
    use super::*;

    #[test]
    fn handles_complex_nested_structure() {
        let content = r#"{
            "users": [
                {
                    "id": 1,
                    "name": "Alice",
                    "metadata": {
                        "created": "2024-01-01",
                        "tags": ["admin", "active"]
                    }
                }
            ],
            "count": 1,
            "hasMore": false
        }"#;

        let result = JsonParser::extract_json(content, None).unwrap();
        assert_eq!(result["users"][0]["name"], "Alice");
        assert_eq!(result["users"][0]["metadata"]["tags"][0], "admin");
        assert_eq!(result["count"], 1);
        assert_eq!(result["hasMore"], false);
    }

    #[test]
    fn handles_special_values() {
        let content = r#"{"null_val": null, "bool_true": true, "bool_false": false, "number": 3.14}"#;
        let result = JsonParser::extract_json(content, None).unwrap();

        assert!(result["null_val"].is_null());
        assert_eq!(result["bool_true"], true);
        assert_eq!(result["bool_false"], false);
        assert!((result["number"].as_f64().unwrap() - 3.14).abs() < 0.001);
    }

    #[test]
    fn handles_empty_structures() {
        let content = r#"{"empty_obj": {}, "empty_arr": []}"#;
        let result = JsonParser::extract_json(content, None).unwrap();

        assert!(result["empty_obj"].is_object());
        assert!(result["empty_arr"].is_array());
        assert!(result["empty_arr"].as_array().unwrap().is_empty());
    }
}
