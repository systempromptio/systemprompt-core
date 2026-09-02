//! Argv redaction for the admin CLI gateway.
//!
//! The gateway logs the argv it forwards, and `admin config secret set` takes
//! the secret as a bare positional — so before this existed every secret set
//! through that route was written to the log store in plaintext.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_logging::sanitize::{REDACTION_PLACEHOLDER, redact_argv};

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_positional_secret_value_is_replaced_and_its_name_kept() {
    let out = redact_argv(&argv(&[
        "admin",
        "config",
        "secret",
        "set",
        "anthropic",
        "sk-live-abc123",
    ]));
    assert_eq!(out[4], "anthropic");
    assert_eq!(out[5], REDACTION_PLACEHOLDER);
}

#[test]
fn an_inline_flag_value_is_replaced_and_the_flag_name_kept() {
    let out = redact_argv(&argv(&["admin", "setup", "--gemini-key=AIzaSyLive"]));
    assert_eq!(out[2], format!("--gemini-key={REDACTION_PLACEHOLDER}"));
}

#[test]
fn a_separated_flag_value_is_replaced_in_the_following_element() {
    let out = redact_argv(&argv(&["admin", "setup", "--db-password", "hunter2"]));
    assert_eq!(out[2], "--db-password");
    assert_eq!(out[3], REDACTION_PLACEHOLDER);
}

#[test]
fn a_credentialed_url_is_replaced_even_when_the_flag_name_looks_harmless() {
    let out = redact_argv(&argv(&[
        "admin",
        "setup",
        "--database-url",
        "postgres://user:pw@host/db",
    ]));
    assert_eq!(out[3], REDACTION_PLACEHOLDER);

    let inline = redact_argv(&argv(&["--database-url=postgres://user:pw@host/db"]));
    assert_eq!(inline[0], format!("--database-url={REDACTION_PLACEHOLDER}"));
}

#[test]
fn ordinary_arguments_survive_untouched() {
    let args = argv(&[
        "admin",
        "setup",
        "--db-host",
        "localhost",
        "--db-port",
        "5432",
    ]);
    assert_eq!(redact_argv(&args), args);
}

#[test]
fn a_short_command_is_not_mistaken_for_the_secret_positional() {
    let args = argv(&["admin", "config", "secret", "set"]);
    assert_eq!(redact_argv(&args), args);
}
