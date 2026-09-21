use systemprompt_marketplace::managed::{ManagedError, normalize_form_text};

#[test]
fn browser_edits_preserve_the_baselines_single_line_ending_convention() {
    assert_eq!(
        normalize_form_text("first\r\nsecond\r\n", "edited\nnext\r\nlast\r")
            .expect("CRLF baseline accepts browser-normalized text"),
        "edited\r\nnext\r\nlast\r\n"
    );
    assert_eq!(
        normalize_form_text("first\rsecond\r", "edited\r\nnext\nlast\r")
            .expect("CR baseline accepts browser-normalized text"),
        "edited\rnext\rlast\r"
    );
    assert_eq!(
        normalize_form_text("first\nsecond\n", "edited\r\nnext\rlast\n")
            .expect("LF baseline accepts browser-normalized text"),
        "edited\nnext\nlast\n"
    );
    assert_eq!(
        normalize_form_text("single line", "edited\r\nnext")
            .expect("a baseline without line endings uses canonical LF"),
        "edited\nnext"
    );
}

#[test]
fn mixed_baseline_requires_an_exact_asset_revision_without_rewriting_bytes() {
    for original in ["first\r\nsecond\n", "first\r\nsecond\r", "first\nsecond\r"] {
        let error = normalize_form_text(original, "replacement")
            .expect_err("mixed source line endings cannot be inferred safely");
        assert!(
            matches!(error, ManagedError::Invalid(message) if message == "Mixed line endings require an exact asset revision")
        );
    }
}

#[test]
fn binary_and_oversized_form_submissions_are_rejected_before_normalization() {
    for (original, submitted) in [("source\0bytes", "edited"), ("source", "edited\0bytes")] {
        let error = normalize_form_text(original, submitted)
            .expect_err("NUL-bearing text requires the exact asset path");
        assert!(
            matches!(error, ManagedError::Invalid(message) if message == "NUL bytes require an exact asset revision")
        );
    }
    let oversized = "x".repeat(1024 * 1024 + 1);
    for (original, submitted) in [
        (oversized.as_str(), "edited"),
        ("source", oversized.as_str()),
    ] {
        let error = normalize_form_text(original, submitted)
            .expect_err("form text is bounded on both sides");
        assert!(
            matches!(error, ManagedError::Invalid(message) if message == "Text edits exceed 1 MiB")
        );
    }
}
