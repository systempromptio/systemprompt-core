//! Tests for JsonParser.

use systemprompt_ai::services::structured_output::parser::JsonParser;

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
        result.unwrap_err();
    }

    #[test]
    fn returns_error_for_empty_content() {
        let result = JsonParser::extract_json("", None);
        result.unwrap_err();
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
        assert!(
            result.get("first").is_some() || result.get("second").is_some(),
            "expected at least one of 'first' or 'second' keys in result"
        );
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
        let content =
            r#"{"null_val": null, "bool_true": true, "bool_false": false, "number": 3.14}"#;
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

mod heuristic_scanning {
    use super::*;

    #[test]
    fn braces_inside_a_string_literal_do_not_close_the_object() {
        let content = "The model said: {\"template\": \"use {placeholder} here\", \"ok\": true} \
                       and then kept talking.";
        let result = JsonParser::extract_json(content, None).unwrap();

        assert_eq!(result["template"], "use {placeholder} here");
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn an_escaped_quote_does_not_terminate_the_string_scan() {
        let content = "prose {\"quoted\": \"she said \\\"hi\\\" loudly\", \"n\": 1} more prose";
        let result = JsonParser::extract_json(content, None).unwrap();

        assert_eq!(result["quoted"], "she said \"hi\" loudly");
        assert_eq!(result["n"], 1);
    }

    #[test]
    fn an_escaped_backslash_before_a_quote_still_closes_the_string() {
        let content = r#"prose {"path": "C:\\dir\\", "n": 2} more"#;
        let result = JsonParser::extract_json(content, None).unwrap();

        assert_eq!(result["path"], "C:\\dir\\");
        assert_eq!(result["n"], 2);
    }

    #[test]
    fn a_nested_array_inside_an_object_is_scanned_to_the_outer_close() {
        let content = "answer: {\"items\": [{\"a\": 1}, {\"b\": [2, 3]}], \"done\": true} end";
        let result = JsonParser::extract_json(content, None).unwrap();

        assert_eq!(result["items"][1]["b"][1], 3);
        assert_eq!(result["done"], true);
    }

    #[test]
    fn an_unbalanced_brace_run_yields_the_no_json_error_rather_than_a_partial_value() {
        let err = JsonParser::extract_json("here it comes: {\"a\": 1, \"b\": ", None)
            .expect_err("an unterminated object must not parse");
        assert!(err.to_string().contains("No valid JSON"), "got {err}");
    }

    #[test]
    fn a_balanced_brace_run_that_is_not_json_is_rejected() {
        let err = JsonParser::extract_json("config { key = value; other = 2 }", None)
            .expect_err("balanced braces are not sufficient — the span must decode as JSON");
        assert!(err.to_string().contains("No valid JSON"), "got {err}");
    }

    #[test]
    fn content_with_no_opening_bracket_at_all_is_rejected() {
        let err = JsonParser::extract_json("no structure here whatsoever", None)
            .expect_err("plain prose has nothing to extract");
        assert!(err.to_string().contains("No valid JSON"), "got {err}");
    }

    #[test]
    fn a_custom_pattern_that_matches_non_json_falls_through_to_the_builtin_patterns() {
        let content = "PREAMBLE::junk\n```json\n{\"from\": \"fence\"}\n```";
        let result = JsonParser::extract_json(content, Some(r"PREAMBLE::(\w+)")).unwrap();

        assert_eq!(
            result["from"], "fence",
            "a custom pattern whose capture is not JSON must not abort the ladder"
        );
    }
}
