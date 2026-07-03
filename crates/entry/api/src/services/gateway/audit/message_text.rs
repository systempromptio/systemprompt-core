//! Flattening of canonical message content into the plain-text form persisted
//! against an audited AI request.

use super::super::protocol::canonical::CanonicalContent;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn flatten_message_content(parts: &[CanonicalContent]) -> String {
    let mut out = String::new();
    for part in parts {
        match part {
            CanonicalContent::Text(t) => push_with_sep(&mut out, t),
            CanonicalContent::Thinking { text, .. } => push_with_sep(&mut out, text),
            CanonicalContent::ToolUse { name, input, .. } => {
                push_with_sep(&mut out, &format!("[tool_use:{name} {input}]"));
            },
            CanonicalContent::ToolResult { content, .. } => {
                for inner in content {
                    if let CanonicalContent::Text(t) = inner {
                        push_with_sep(&mut out, t);
                    }
                }
            },
            CanonicalContent::Image(_) => {},
        }
    }
    out
}

fn push_with_sep(out: &mut String, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(fragment);
}
