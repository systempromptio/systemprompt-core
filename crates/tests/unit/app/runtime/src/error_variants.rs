//! Tests for every `RuntimeError` variant: Display output shape and basic
//! construction. Also exercises `RuntimeResult<T>` as a type alias.

use systemprompt_runtime::{RuntimeError, RuntimeResult};

fn ok_result() -> RuntimeResult<u32> {
    Ok(42)
}

fn err_result(e: RuntimeError) -> RuntimeResult<u32> {
    Err(e)
}

#[test]
fn runtime_result_ok_unwraps() {
    assert_eq!(ok_result().unwrap(), 42);
}

#[test]
fn runtime_result_err_is_err() {
    let r = err_result(RuntimeError::EmptyDatabaseUrl);
    assert!(r.is_err());
}

#[test]
fn empty_database_url_message() {
    let err = RuntimeError::EmptyDatabaseUrl;
    let msg = err.to_string();
    assert!(msg.contains("empty"), "got: {msg}");
}

#[test]
fn database_not_found_message_contains_path() {
    let err = RuntimeError::DatabaseNotFound {
        path: "/data/app.db".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/data/app.db"), "got: {msg}");
    assert!(
        msg.contains("not found") || msg.contains("Not found"),
        "got: {msg}"
    );
}

#[test]
fn database_not_found_message_contains_setup_hint() {
    let err = RuntimeError::DatabaseNotFound {
        path: "/x/y.db".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("setup"), "got: {msg}");
}

#[test]
fn database_not_file_message_contains_path() {
    let err = RuntimeError::DatabaseNotFile {
        path: "/some/dir".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/some/dir"), "got: {msg}");
    assert!(msg.contains("not a file"), "got: {msg}");
}

#[test]
fn system_admin_not_found_message_contains_username() {
    let err = RuntimeError::SystemAdminNotFound {
        username: "root".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("root"), "got: {msg}");
    assert!(
        msg.contains("not found") || msg.contains("bootstrap"),
        "got: {msg}"
    );
}

#[test]
fn system_admin_inactive_message_contains_username() {
    let err = RuntimeError::SystemAdminInactive {
        username: "inactive_user".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("inactive_user"), "got: {msg}");
    assert!(
        msg.contains("not active") || msg.contains("inactive") || msg.contains("active"),
        "got: {msg}"
    );
}

#[test]
fn system_admin_missing_role_message_contains_username() {
    let err = RuntimeError::SystemAdminMissingRole {
        username: "norole_user".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("norole_user"), "got: {msg}");
    assert!(msg.contains("role") || msg.contains("admin"), "got: {msg}");
}

#[test]
fn internal_error_message_contains_detail() {
    let err = RuntimeError::Internal("socket closed".to_string());
    let msg = err.to_string();
    assert!(msg.contains("socket closed"), "got: {msg}");
}

#[test]
fn internal_error_message_has_internal_prefix() {
    let err = RuntimeError::Internal("test detail".to_string());
    let msg = err.to_string();
    assert!(msg.contains("internal"), "got: {msg}");
}

#[test]
fn error_debug_is_non_empty() {
    let err = RuntimeError::EmptyDatabaseUrl;
    let dbg = format!("{err:?}");
    assert!(!dbg.is_empty());
    assert!(dbg.contains("EmptyDatabaseUrl"), "got: {dbg}");
}

#[test]
fn database_not_found_debug_contains_variant() {
    let err = RuntimeError::DatabaseNotFound {
        path: "/tmp/x.db".to_string(),
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("DatabaseNotFound"), "got: {dbg}");
}

#[test]
fn system_admin_not_found_debug_contains_variant() {
    let err = RuntimeError::SystemAdminNotFound {
        username: "alice".to_string(),
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("SystemAdminNotFound"), "got: {dbg}");
    assert!(dbg.contains("alice"), "got: {dbg}");
}

#[test]
fn all_plain_variants_format_without_panic() {
    let variants: Vec<RuntimeError> = vec![
        RuntimeError::EmptyDatabaseUrl,
        RuntimeError::DatabaseNotFound {
            path: "/p".to_string(),
        },
        RuntimeError::DatabaseNotFile {
            path: "/d".to_string(),
        },
        RuntimeError::SystemAdminNotFound {
            username: "u".to_string(),
        },
        RuntimeError::SystemAdminInactive {
            username: "u".to_string(),
        },
        RuntimeError::SystemAdminMissingRole {
            username: "u".to_string(),
        },
        RuntimeError::Internal("msg".to_string()),
    ];

    for v in variants {
        let s = v.to_string();
        assert!(!s.is_empty(), "empty Display for {v:?}");
    }
}
