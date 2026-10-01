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
fn unsupported_database_url_message_names_the_schemes() {
    let msg = RuntimeError::UnsupportedDatabaseUrl.to_string();
    assert!(msg.contains("postgres://"), "got: {msg}");
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
fn services_bundle_not_cached_names_the_source() {
    let err = RuntimeError::ServicesBundleNotCached {
        name: "acme".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("acme") && msg.contains("no cached fetch state"),
        "got: {msg}"
    );
}

#[test]
fn storage_read_back_names_the_root() {
    let err = RuntimeError::StorageReadBack {
        path: std::path::PathBuf::from("/srv/storage"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/srv/storage"), "got: {msg}");
}

#[test]
fn error_debug_is_non_empty() {
    let err = RuntimeError::EmptyDatabaseUrl;
    let dbg = format!("{err:?}");
    assert!(!dbg.is_empty());
    assert!(dbg.contains("EmptyDatabaseUrl"), "got: {dbg}");
}


#[test]
fn all_plain_variants_format_without_panic() {
    let variants: Vec<(RuntimeError, &str)> = vec![
        (RuntimeError::EmptyDatabaseUrl, "DATABASE_URL is empty"),
        (RuntimeError::UnsupportedDatabaseUrl, "postgresql://"),
        (
            RuntimeError::SystemAdminNotFound {
                username: "u".to_string(),
            },
            "was not found",
        ),
        (
            RuntimeError::SystemAdminInactive {
                username: "u".to_string(),
            },
            "not active",
        ),
        (
            RuntimeError::SystemAdminMissingRole {
                username: "u".to_string(),
            },
            "'admin' role",
        ),
    ];

    for (v, marker) in variants {
        let s = v.to_string();
        assert!(
            s.contains(marker),
            "Display for {v:?} missing {marker:?}: {s}"
        );
    }
}
