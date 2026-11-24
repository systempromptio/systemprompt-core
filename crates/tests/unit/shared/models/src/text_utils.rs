use std::borrow::Cow;

use serde_json::json;
use systemprompt_models::text::{
    chunk_text, json_without_nul, truncate_with_ellipsis, without_nul,
};

mod nul_tests {
    use super::*;

    #[test]
    fn text_without_a_nul_is_borrowed_unchanged() {
        assert!(matches!(without_nul("plain é"), Cow::Borrowed("plain é")));
    }

    #[test]
    fn every_nul_in_text_becomes_the_replacement_character() {
        assert_eq!(without_nul("\0a\0\0b"), "\u{FFFD}a\u{FFFD}\u{FFFD}b");
    }

    #[test]
    fn json_without_a_nul_is_borrowed() {
        let value = json!({"a": ["b", 1, null, true]});
        assert!(matches!(json_without_nul(&value), Cow::Borrowed(_)));
    }

    #[test]
    fn json_leaves_and_keys_are_rewritten_at_any_depth() {
        let value = json!({"k\0": [{"deep": "x\0y"}, 7, null, false], "kept": "z"});
        assert_eq!(
            json_without_nul(&value).into_owned(),
            json!({"k\u{FFFD}": [{"deep": "x\u{FFFD}y"}, 7, null, false], "kept": "z"})
        );
    }

    #[test]
    fn an_escaped_nul_parsed_from_json_text_is_rewritten() {
        let value = serde_json::from_str(r#"{"a":"\u0000"}"#).expect("json");
        assert_eq!(*json_without_nul(&value), json!({"a": "\u{FFFD}"}));
    }
}

mod chunk_text_tests {
    use super::*;

    #[test]
    fn empty_input_yields_single_empty_chunk() {
        assert_eq!(chunk_text("", 100), vec![String::new()]);
    }

    #[test]
    fn short_text_is_one_chunk() {
        assert_eq!(chunk_text("hello\nworld", 100), vec!["hello\nworld"]);
    }

    #[test]
    fn splits_on_line_boundaries_under_limit() {
        let text = "aaaa\nbbbb\ncccc";
        let chunks = chunk_text(text, 6);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(!chunk.contains("\naaaa\nbbbb"));
        }
        assert_eq!(chunks.join("\n"), text);
    }

    #[test]
    fn single_oversized_line_is_emitted_whole() {
        let line = "x".repeat(50);
        assert_eq!(chunk_text(&line, 10), vec![line]);
    }
}

mod truncate_with_ellipsis_tests {
    use super::*;

    #[test]
    fn short_text_unchanged() {
        let result = truncate_with_ellipsis("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn exact_length_unchanged() {
        let result = truncate_with_ellipsis("12345", 5);
        assert_eq!(result, "12345");
    }

    #[test]
    fn over_limit_gets_ellipsis() {
        let result = truncate_with_ellipsis("123456", 5);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 5 + 3);
    }

    #[test]
    fn empty_string_unchanged() {
        let result = truncate_with_ellipsis("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn single_char_within_limit() {
        let result = truncate_with_ellipsis("x", 1);
        assert_eq!(result, "x");
    }

    #[test]
    fn long_text_truncated_to_limit() {
        let long = "a".repeat(1000);
        let result = truncate_with_ellipsis(&long, 100);
        assert!(result.len() <= 103);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn zero_max_len_produces_ellipsis_only() {
        let result = truncate_with_ellipsis("hello", 0);
        assert_eq!(result, "...");
    }

    #[test]
    fn max_len_of_3_truncates_to_ellipsis_only() {
        let result = truncate_with_ellipsis("hello world", 3);
        assert_eq!(result, "...");
    }

    #[test]
    fn max_len_of_4_keeps_one_char_plus_ellipsis() {
        let result = truncate_with_ellipsis("hello world", 4);
        assert!(result.starts_with('h'));
        assert!(result.ends_with("..."));
    }

    #[test]
    fn unicode_multibyte_does_not_panic() {
        let unicode_text = "Hello \u{1F600}\u{1F600}\u{1F600} world";
        let result = truncate_with_ellipsis(unicode_text, 10);
        assert!(result.len() <= 13);
    }

    #[test]
    fn cjk_characters_boundary_safe() {
        let cjk = "\u{4E16}\u{754C}\u{4F60}\u{597D}";
        let result = truncate_with_ellipsis(cjk, 6);
        assert!(result.ends_with("...") || result == cjk);
    }

    #[test]
    fn newlines_in_text_preserved() {
        let text = "line1\nline2\nline3";
        let result = truncate_with_ellipsis(text, 100);
        assert_eq!(result, text);
    }

    #[test]
    fn newlines_in_truncated_text() {
        let text = "line1\nline2\nline3\nline4\nline5";
        let result = truncate_with_ellipsis(text, 15);
        assert!(result.ends_with("..."));
    }
}
