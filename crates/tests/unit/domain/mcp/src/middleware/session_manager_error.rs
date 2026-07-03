//! Tests for [`DatabaseSessionManagerError`] Display and Error source.

use systemprompt_mcp::McpDomainError;
use systemprompt_mcp::middleware::session_handler::DatabaseSessionManagerError;

#[test]
fn session_not_found_display() {
    let e = DatabaseSessionManagerError::SessionNotFound("sess-abc".to_owned());
    let s = e.to_string();
    assert!(s.contains("sess-abc"), "got: {s}");
}

#[test]
fn session_expired_display() {
    let e = DatabaseSessionManagerError::SessionExpired("sess-xyz".to_owned());
    let s = e.to_string();
    assert!(s.contains("sess-xyz"), "got: {s}");
}

#[test]
fn session_needs_reconnect_display() {
    let e = DatabaseSessionManagerError::SessionNeedsReconnect("sess-r".to_owned());
    let s = e.to_string();
    assert!(s.contains("sess-r") || s.contains("reconnect"), "got: {s}");
}

#[test]
fn database_variant_display() {
    let inner = McpDomainError::Internal("db fail".to_owned());
    let e = DatabaseSessionManagerError::Database(inner);
    let s = e.to_string();
    assert!(
        s.contains("db fail") || s.contains("Database") || s.contains("database"),
        "got: {s}"
    );
}

#[test]
fn database_variant_source_is_some() {
    use std::error::Error;
    let inner = McpDomainError::Internal("src".to_owned());
    let e = DatabaseSessionManagerError::Database(inner);
    let src = e.source().expect("database variant has a source");
    assert!(src.to_string().contains("src"));
}

#[test]
fn session_not_found_source_is_none() {
    use std::error::Error;
    let e = DatabaseSessionManagerError::SessionNotFound("x".to_owned());
    assert!(e.source().is_none());
}

#[test]
fn session_expired_source_is_none() {
    use std::error::Error;
    let e = DatabaseSessionManagerError::SessionExpired("x".to_owned());
    assert!(e.source().is_none());
}

#[test]
fn session_needs_reconnect_source_is_none() {
    use std::error::Error;
    let e = DatabaseSessionManagerError::SessionNeedsReconnect("x".to_owned());
    assert!(e.source().is_none());
}

#[test]
fn debug_format_all_variants() {
    let variants: Vec<(DatabaseSessionManagerError, &str)> = vec![
        (
            DatabaseSessionManagerError::SessionNotFound("a".to_owned()),
            "SessionNotFound",
        ),
        (
            DatabaseSessionManagerError::SessionExpired("b".to_owned()),
            "SessionExpired",
        ),
        (
            DatabaseSessionManagerError::SessionNeedsReconnect("c".to_owned()),
            "SessionNeedsReconnect",
        ),
        (
            DatabaseSessionManagerError::Database(McpDomainError::Internal("d".to_owned())),
            "Database",
        ),
    ];
    for (v, name) in variants {
        let s = format!("{v:?}");
        assert!(s.contains(name), "got: {s}");
    }
}
