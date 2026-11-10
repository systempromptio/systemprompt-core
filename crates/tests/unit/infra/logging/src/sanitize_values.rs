//! Content-based scrubbing and size caps for log field values and messages.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_logging::sanitize::{
    MAX_FIELD_BYTES, MAX_MESSAGE_BYTES, REDACTION_PLACEHOLDER, sanitize_field, sanitize_message,
    scrub_value,
};

#[test]
fn userinfo_in_a_url_value_is_redacted_and_the_host_kept() {
    let out = sanitize_field("url", "https://svc:s3cr3t@authz.internal/decide");
    assert_eq!(
        out,
        format!("https://{REDACTION_PLACEHOLDER}@authz.internal/decide")
    );
}

#[test]
fn userinfo_inside_an_error_message_is_redacted() {
    let out = sanitize_field(
        "error",
        "error connecting to postgres://admin:pw@db:5432/app: refused",
    );
    assert!(!out.contains("pw@"), "{out}");
    assert!(out.contains("@db:5432/app: refused"), "{out}");
}

#[test]
fn secret_query_parameters_are_redacted_and_others_kept() {
    let out = scrub_value("https://h/x?page=2&api_key=abc123&key=k9&token=t1#frag");
    assert_eq!(
        out,
        format!(
            "https://h/x?page=2&api_key={p}&key={p}&token={p}#frag",
            p = REDACTION_PLACEHOLDER
        )
    );
}

#[test]
fn plain_values_pass_through_unchanged() {
    for value in ["hello world", "a@b.com", "ratio=0.5&n=3", "file:///tmp/x"] {
        assert_eq!(scrub_value(value), value);
    }
}

#[test]
fn a_secret_named_field_is_blanked_whatever_its_value() {
    assert_eq!(sanitize_field("password", "plain"), REDACTION_PLACEHOLDER);
}

#[test]
fn field_values_are_capped_on_a_char_boundary_with_a_marker() {
    let value = "é".repeat(MAX_FIELD_BYTES);
    let out = sanitize_field("detail", &value);
    let (kept, marker) = out.split_once('…').expect("truncation marker");
    assert!(kept.len() <= MAX_FIELD_BYTES);
    assert!(kept.chars().all(|c| c == 'é'));
    assert_eq!(
        marker,
        format!("[truncated {} bytes]", value.len() - kept.len())
    );
}

#[test]
fn messages_are_capped_and_scrubbed() {
    let message = format!(
        "calling https://u:p@h/ {}",
        "x".repeat(MAX_MESSAGE_BYTES * 2)
    );
    let out = sanitize_message(&message);
    assert!(!out.contains("u:p@"), "credentials must be scrubbed");
    assert!(out.len() < message.len());
    assert!(out.contains("[truncated "));
}
