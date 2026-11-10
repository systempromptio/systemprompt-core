//! Shared field-value sanitization for the console and database log sinks.
//!
//! Both [`crate::services::FilterSystemFields`] (console) and the database
//! field visitor route values through here so the two sinks cannot drift on
//! what counts as a secret.
//!
//! A field is blanked when its *name* marks it secret. Every other string
//! value is scrubbed by content: the userinfo of any `scheme://user:pass@host`
//! and the value of any secret-named query parameter are replaced with
//! [`REDACTION_PLACEHOLDER`], so a URL or an error that echoes one never
//! carries its credentials into a sink. Field values and messages are capped
//! at [`MAX_FIELD_BYTES`] and [`MAX_MESSAGE_BYTES`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const REDACTION_PLACEHOLDER: &str = "[REDACTED]";

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

const REDACT_SUFFIXES: &[&str] = &["_cert", "_pem", "_key", "-key"];

const REDACT_EXACT: &[&str] = &["auth", "cert", "pem"];

pub const MAX_FIELD_BYTES: usize = 2048;

pub const MAX_MESSAGE_BYTES: usize = 8192;

const SECRET_QUERY_KEYS: &[&str] = &["key", "sig", "signature", "code", "access_token"];

pub fn is_redacted(field_name: &str) -> bool {
    let lower = field_name.to_ascii_lowercase();
    REDACT_SUBSTRINGS.iter().any(|s| lower.contains(s))
        || REDACT_SUFFIXES.iter().any(|s| lower.ends_with(s))
        || REDACT_EXACT.iter().any(|e| lower == *e)
}

pub(crate) fn is_system_sentinel(rendered: &str) -> bool {
    rendered == "system" || rendered == "\"system\""
}

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

const SECRET_POSITIONAL_COMMAND: &[&str] = &["admin", "config", "secret", "set"];

pub fn sanitize_field(name: &str, value: &str) -> String {
    if is_redacted(name) {
        return REDACTION_PLACEHOLDER.to_owned();
    }
    cap(scrub_value(value), MAX_FIELD_BYTES)
}

pub fn sanitize_message(message: &str) -> String {
    cap(scrub_value(message), MAX_MESSAGE_BYTES)
}

pub fn scrub_value(value: &str) -> String {
    scrub_query_secrets(&scrub_userinfo(value))
}

const fn ends_token(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b'"' | b'\'' | b'<' | b'>' | b'`' | b')' | b']')
}

fn scrub_userinfo(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut copied = 0;
    let mut search = 0;
    while let Some(found) = value[search..].find("://") {
        let start = search + found + 3;
        let end = bytes[start..]
            .iter()
            .position(|&b| ends_token(b) || matches!(b, b'/' | b'?' | b'#'))
            .map_or(bytes.len(), |p| start + p);
        if let Some(at) = value[start..end].rfind('@') {
            out.push_str(&value[copied..start]);
            out.push_str(REDACTION_PLACEHOLDER);
            copied = start + at;
        }
        search = end.max(start);
    }
    out.push_str(&value[copied..]);
    out
}

fn is_secret_query_key(key: &str) -> bool {
    is_redacted(key)
        || SECRET_QUERY_KEYS
            .iter()
            .any(|k| key.eq_ignore_ascii_case(k))
}

fn scrub_query_secrets(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        if matches!(bytes[i], b'?' | b'&') {
            let key_start = i + 1;
            let key_end = bytes[key_start..]
                .iter()
                .position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')))
                .map_or(bytes.len(), |p| key_start + p);
            if key_end > key_start
                && bytes.get(key_end) == Some(&b'=')
                && is_secret_query_key(&value[key_start..key_end])
            {
                let value_start = key_end + 1;
                let value_end = bytes[value_start..]
                    .iter()
                    .position(|&b| ends_token(b) || matches!(b, b'&' | b'#'))
                    .map_or(bytes.len(), |p| value_start + p);
                out.push_str(&value[copied..value_start]);
                out.push_str(REDACTION_PLACEHOLDER);
                copied = value_end;
                i = value_end;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&value[copied..]);
    out
}

fn cap(value: String, max: usize) -> String {
    if value.len() <= max {
        return value;
    }
    let mut cut = max;
    while !value.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…[truncated {} bytes]", &value[..cut], value.len() - cut)
}

fn has_embedded_credentials(value: &str) -> bool {
    value
        .split_once("://")
        .is_some_and(|(_, rest)| rest.split_once('@').is_some_and(|(u, _)| !u.contains('/')))
}

pub fn redact_argv(args: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(args.len());
    let mut redact_next = false;

    for arg in args {
        if redact_next {
            redact_next = false;
            out.push(REDACTION_PLACEHOLDER.to_owned());
            continue;
        }

        if let Some(flag) = arg.strip_prefix('-') {
            let flag = flag.trim_start_matches('-');
            if let Some((name, value)) = flag.split_once('=') {
                if is_redacted(name) || has_embedded_credentials(value) {
                    let dashes = &arg[..arg.len() - flag.len()];
                    out.push(format!("{dashes}{name}={REDACTION_PLACEHOLDER}"));
                } else {
                    out.push(arg.clone());
                }
            } else {
                redact_next = is_redacted(flag);
                out.push(arg.clone());
            }
            continue;
        }

        if has_embedded_credentials(arg) {
            out.push(REDACTION_PLACEHOLDER.to_owned());
        } else {
            out.push(arg.clone());
        }
    }

    if args.len() > SECRET_POSITIONAL_COMMAND.len() + 1
        && args
            .iter()
            .zip(SECRET_POSITIONAL_COMMAND)
            .all(|(a, expected)| a.as_str() == *expected)
    {
        REDACTION_PLACEHOLDER.clone_into(&mut out[SECRET_POSITIONAL_COMMAND.len() + 1]);
    }

    out
}
