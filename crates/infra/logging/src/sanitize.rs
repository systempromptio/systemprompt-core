//! Shared field-value sanitization for the console and database log sinks.
//!
//! Both [`crate::services::FilterSystemFields`] (console) and the database
//! field visitor route values through here so the two sinks cannot drift on
//! what counts as a secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) const REDACTION_PLACEHOLDER: &str = "[REDACTED]";

/// Substrings (matched case-insensitively anywhere in the name) that mark a
/// field as carrying a secret, covering `client_secret`, `auth_token`,
/// `id_token`, `x-api-key`, `set-cookie`, and similar.
const REDACT_SUBSTRINGS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "cookie",
    "authorization",
    "credential",
    "api_key",
    "apikey",
    "private_key",
    "bearer",
];

/// Suffixes that mark a field as carrying credential material.
const REDACT_SUFFIXES: &[&str] = &["_cert", "_pem"];

/// Field names redacted on an exact (case-insensitive) match.
const REDACT_EXACT: &[&str] = &["auth", "cert", "pem"];

pub(crate) fn is_redacted(field_name: &str) -> bool {
    let lower = field_name.to_ascii_lowercase();
    REDACT_SUBSTRINGS.iter().any(|s| lower.contains(s))
        || REDACT_SUFFIXES.iter().any(|s| lower.ends_with(s))
        || REDACT_EXACT.iter().any(|e| lower == *e)
}

/// Whether `rendered` is the `system` attribution sentinel that
/// [`SystemSpan`](crate::SystemSpan) stamps onto internal spans. The console
/// sink drops it as noise; the database sink keeps it for attribution queries.
/// `rendered` may be a bare `system` or a `Debug`-quoted `"system"`.
pub(crate) fn is_system_sentinel(rendered: &str) -> bool {
    rendered == "system" || rendered == "\"system\""
}

/// Escapes newlines and other control characters so `value` stays on a single
/// line for the line-oriented console sink. The database sink stores JSON and
/// does not need this (serde escapes on serialization).
pub(crate) fn escape_control(value: &str) -> String {
    if !value.chars().any(char::is_control) {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:04x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
