//! Tests for [`DatabaseSessionHandlerError`] Display and Error source.

use systemprompt_mcp::McpDomainError;
use systemprompt_mcp::middleware::session_handler::DatabaseSessionHandlerError;

#[test]
fn session_not_found_display() {
    let e = DatabaseSessionHandlerError::SessionNotFound("sess-abc".to_owned());
    let s = e.to_string();
    assert!(s.contains("sess-abc"), "got: {s}");
}

#[test]
fn session_expired_display() {
    let e = DatabaseSessionHandlerError::SessionExpired("sess-xyz".to_owned());
    let s = e.to_string();
    assert!(s.contains("sess-xyz"), "got: {s}");
}

#[test]
fn session_needs_reconnect_display() {
    let e = DatabaseSessionHandlerError::SessionNeedsReconnect("sess-r".to_owned());
    let s = e.to_string();
    assert!(s.contains("sess-r") || s.contains("reconnect"), "got: {s}");
}

#[test]
fn database_variant_display() {
    let inner = McpDomainError::ServiceRowMissing {
        service: "db fail".to_owned(),
    };
    let e = DatabaseSessionHandlerError::Database(inner);
    let s = e.to_string();
    assert!(
        s.contains("db fail") || s.contains("Database") || s.contains("database"),
        "got: {s}"
    );
}

#[test]
fn database_variant_source_is_some() {
    use std::error::Error;
    let inner = McpDomainError::ServiceRowMissing {
        service: "src".to_owned(),
    };
    let e = DatabaseSessionHandlerError::Database(inner);
    let src = e.source().expect("database variant has a source");
    assert!(src.to_string().contains("src"));
}

#[test]
fn session_not_found_source_is_none() {
    use std::error::Error;
    let e = DatabaseSessionHandlerError::SessionNotFound("x".to_owned());
    assert!(e.source().is_none());
}

#[test]
fn session_expired_source_is_none() {
    use std::error::Error;
    let e = DatabaseSessionHandlerError::SessionExpired("x".to_owned());
    assert!(e.source().is_none());
}

#[test]
fn session_needs_reconnect_source_is_none() {
    use std::error::Error;
    let e = DatabaseSessionHandlerError::SessionNeedsReconnect("x".to_owned());
    assert!(e.source().is_none());
}

#[test]
fn debug_format_all_variants() {
    let variants: Vec<(DatabaseSessionHandlerError, &str)> = vec![
        (
            DatabaseSessionHandlerError::SessionNotFound("a".to_owned()),
            "SessionNotFound",
        ),
        (
            DatabaseSessionHandlerError::SessionExpired("b".to_owned()),
            "SessionExpired",
        ),
        (
            DatabaseSessionHandlerError::SessionNeedsReconnect("c".to_owned()),
            "SessionNeedsReconnect",
        ),
        (
            DatabaseSessionHandlerError::Database(McpDomainError::ServiceRowMissing {
                service: "d".to_owned(),
            }),
            "Database",
        ),
    ];
    for (v, name) in variants {
        let s = format!("{v:?}");
        assert!(s.contains(name), "got: {s}");
    }
}
