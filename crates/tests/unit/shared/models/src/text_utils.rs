use systemprompt_models::text::truncate_with_ellipsis;

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
