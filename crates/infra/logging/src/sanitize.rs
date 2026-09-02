//! Shared field-value sanitization for the console and database log sinks.
//!
//! Both [`crate::services::FilterSystemFields`] (console) and the database
//! field visitor route values through here so the two sinks cannot drift on
//! what counts as a secret.
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
            match flag.split_once('=') {
                Some((name, value)) => {
                    if is_redacted(name) || has_embedded_credentials(value) {
                        let dashes = &arg[..arg.len() - flag.len()];
                        out.push(format!("{dashes}{name}={REDACTION_PLACEHOLDER}"));
                    } else {
                        out.push(arg.clone());
                    }
                },
                None => {
                    redact_next = is_redacted(flag);
                    out.push(arg.clone());
                },
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
        out[SECRET_POSITIONAL_COMMAND.len() + 1] = REDACTION_PLACEHOLDER.to_owned();
    }

    out
}
