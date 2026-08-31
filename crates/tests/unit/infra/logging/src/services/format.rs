//! Tests for `FilterSystemFields` and its visitor.
//!
//! The visitor is exercised by driving it through a real `tracing` event so
//! we touch the actual `Visit` impls (`record_str`, `record_debug`) without
//! reaching for crate-private constructors.

use std::sync::{Arc, Mutex};
use systemprompt_logging::FilterSystemFields;
use tracing::{Subscriber, info, info_span};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Clone, Default)]
struct CapturingWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl CapturingWriter {
    fn contents(&self) -> String {
        let buf = self.buffer.lock().expect("buffer poisoned");
        String::from_utf8(buf.clone()).expect("utf8 in captured logs")
    }
}

impl std::io::Write for CapturingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer
            .lock()
            .expect("buffer poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturingWriter {
    type Writer = CapturingWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn make_subscriber(writer: CapturingWriter) -> impl Subscriber + Send + Sync {
    let layer = tracing_subscriber::fmt::layer()
        .fmt_fields(FilterSystemFields::new())
        .with_target(false)
        .with_writer(writer)
        .with_ansi(false);
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("trace"))
        .with(layer)
}

#[test]
fn filter_is_copy_default_debug() {
    let f = FilterSystemFields::new();
    let copied = f;
    let _debug = format!("{f:?} {copied:?}");
    let _default = FilterSystemFields;
}

#[test]
fn record_str_drops_literal_system_value() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(actor = "system", message = "should-not-emit-actor");
    });

    let logs = writer.contents();
    // The 'actor=system' field must not appear (record_str early-returns on
    // "system").
    assert!(!logs.contains("actor="));
    // The 'message' field WAS emitted (and logged as the event message).
    assert!(logs.contains("should-not-emit-actor"));
}

#[test]
fn record_debug_drops_system_debug_value() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        // Use a non-string field whose Debug repr is exactly "system" or "\"system\"".
        info!(owner = ?"system", body = "hello");
    });

    let logs = writer.contents();
    assert!(!logs.contains("owner="));
    assert!(logs.contains("hello"));
}

#[test]
fn other_fields_are_preserved_and_space_separated() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(user_id = "alice", count = 42, "structured");
    });

    let logs = writer.contents();
    assert!(logs.contains("user_id="));
    assert!(logs.contains("alice"));
    assert!(logs.contains("count=42"));
}

#[test]
fn span_fields_also_filtered() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        let span = info_span!("op", actor = "system", tenant = "acme");
        let _enter = span.enter();
        info!("inside-span");
    });

    let logs = writer.contents();
    assert!(logs.contains("inside-span"));
    assert!(logs.contains("tenant"));
}

#[test]
fn multi_line_error_field_renders_single_line() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        let err = "* upstream call failed\n  caused by: 400 Bad Request\n";
        tracing::warn!(
            provider = "anthropic",
            model = "claude-x",
            error = %err,
            "gateway upstream call failed"
        );
    });

    let logs = writer.contents();
    assert!(logs.contains("gateway upstream call failed"));
    assert!(logs.contains("error="));
    assert!(logs.contains("\\n"));
    assert_eq!(logs.matches('\n').count(), 1);
}

#[test]
fn secret_named_fields_are_redacted_on_console() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(
            api_key = "sk-live-123",
            authorization = "Bearer abc",
            normal = "ok",
            "secrets"
        );
    });

    let logs = writer.contents();
    assert!(!logs.contains("sk-live-123"));
    assert!(!logs.contains("Bearer abc"));
    assert!(logs.contains("[REDACTED]"));
    assert!(logs.contains("normal="));
}

#[test]
fn numeric_fields_are_not_redacted_by_name_on_console() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(
            oauth_tokens = 49_u64,
            oauth_codes = 0_u64,
            token_valid = true,
            access_token = "sk-live-123",
            "cleanup finished"
        );
    });

    let logs = writer.contents();
    assert!(
        logs.contains("oauth_tokens=49"),
        "a delete count whose name contains `token` must survive: {logs}"
    );
    assert!(logs.contains("oauth_codes=0"), "{logs}");
    assert!(logs.contains("token_valid=true"), "{logs}");
    assert!(
        !logs.contains("sk-live-123") && logs.contains("[REDACTED]"),
        "string secrets are still redacted: {logs}"
    );
}

#[test]
fn control_chars_escape_and_suffix_exact_redaction_rules() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(
            payload = %"a\rb\tc\u{7}d",
            tls_cert = "PEMDATA",
            auth = "basic xyz",
            author = "mary",
            "controls"
        );
    });

    let logs = writer.contents();
    assert!(logs.contains("\\r"));
    assert!(logs.contains("\\t"));
    assert!(logs.contains("\\u{0007}"));
    assert!(!logs.contains("PEMDATA"), "_cert suffix must be redacted");
    assert!(!logs.contains("basic xyz"), "exact 'auth' must be redacted");
    assert!(
        logs.contains("author=") && logs.contains("mary"),
        "'author' must not match the exact 'auth' rule"
    );
}

// The redaction rules are a list of dangerous field names, and the tests above
// exercise four of them. Removing any other entry — `cookie`, `password`,
// `bearer` — would leave every one of those tests green while that class of
// secret starts landing in the console and the database. This drives each
// entry so a shrinking list fails rather than passing quietly.
//
// The names below deliberately duplicate the source list. That duplication is
// the mechanism: the test only detects a removal because it holds its own copy.
#[test]
fn every_redaction_rule_actually_redacts() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(
            password = "pw-canary",
            passwd = "passwd-canary",
            secret = "secret-canary",
            token = "token-canary",
            cookie = "cookie-canary",
            authorization = "authz-canary",
            credential = "credential-canary",
            api_key = "apikey-underscore-canary",
            apikey = "apikey-canary",
            private_key = "privatekey-canary",
            bearer = "bearer-canary",
            "every rule"
        );
    });

    let logs = writer.contents();
    for canary in [
        "pw-canary",
        "passwd-canary",
        "secret-canary",
        "token-canary",
        "cookie-canary",
        "authz-canary",
        "credential-canary",
        "apikey-underscore-canary",
        "apikey-canary",
        "privatekey-canary",
        "bearer-canary",
    ] {
        assert!(
            !logs.contains(canary),
            "{canary} reached the log; its redaction rule is no longer matching: {logs}"
        );
    }
}

// Why: the rules match a substring, so a field that merely contains a
// dangerous word is redacted too. That is deliberate — `user_password_hash`
// should not be logged either — and worth pinning so the matching is not
// narrowed to exact names.
#[test]
fn a_field_containing_a_dangerous_word_is_redacted_too() {
    let writer = CapturingWriter::default();
    let subscriber = make_subscriber(writer.clone());

    tracing::subscriber::with_default(subscriber, || {
        info!(
            user_password_hash = "hash-canary",
            refresh_token_value = "refresh-canary",
            session_cookie_jar = "jar-canary",
            "substring rules"
        );
    });

    let logs = writer.contents();
    for canary in ["hash-canary", "refresh-canary", "jar-canary"] {
        assert!(
            !logs.contains(canary),
            "{canary} reached the log despite a dangerous word in its field name: {logs}"
        );
    }
}
