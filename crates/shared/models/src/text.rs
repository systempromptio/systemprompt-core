//! Text-formatting helpers.
//!
//! [`truncate_with_ellipsis`] shortens a string to a maximum byte
//! length, appending `...` and snapping to a UTF-8 character boundary
//! so the result is never split mid-codepoint.

pub fn truncate_with_ellipsis(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let truncated_len = max_len.saturating_sub(3);
    let boundary = find_char_boundary(text, truncated_len);
    format!("{}...", &text[..boundary])
}

fn find_char_boundary(s: &str, target: usize) -> usize {
    if target >= s.len() {
        return s.len();
    }

    let mut boundary = target;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}
