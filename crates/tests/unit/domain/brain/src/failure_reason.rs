//! A failure's reason is written to a TEXT column, which refuses a NUL
//! byte; the scrub replaces each one so the attempt is always recorded.

use systemprompt_brain::models::NewFailure;

#[test]
fn nul_bytes_are_replaced_and_everything_else_is_kept() {
    assert_eq!(
        NewFailure::scrub_reason("upstream said \0bad\0"),
        "upstream said \u{FFFD}bad\u{FFFD}"
    );
    assert_eq!(NewFailure::scrub_reason("plain"), "plain");
    assert_eq!(NewFailure::scrub_reason(""), "");
}
