//! Unit tests for `cloud::profile::redact_database_url`.
//!
//! The helper hides credentials in a connection string while preserving the
//! protocol prefix and the host/database suffix.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::redact_database_url;

#[test]
fn redacts_credentials_between_protocol_and_host() {
    let out = redact_database_url("postgres://user:pass@localhost:5432/db");
    assert_eq!(out, "postgres://[REDACTED]@localhost:5432/db");
    assert!(!out.contains("user"));
    assert!(!out.contains("pass"));
}

#[test]
fn passes_through_url_without_credentials() {
    let url = "postgres://localhost:5432/db";
    assert_eq!(redact_database_url(url), url);
}

#[test]
fn passes_through_string_without_protocol() {
    let raw = "not-a-url";
    assert_eq!(redact_database_url(raw), raw);
}

#[test]
fn preserves_host_and_database_suffix() {
    let out = redact_database_url("mysql://admin:secret@db.internal:3306/app");
    assert!(out.starts_with("mysql://[REDACTED]@"));
    assert!(out.ends_with("@db.internal:3306/app"));
}
