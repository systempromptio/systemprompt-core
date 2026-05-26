use std::fs;

use systemprompt_runtime::{RuntimeError, validate_database_path};
use tempfile::tempdir;

#[test]
fn accepts_postgres_url() {
    validate_database_path("postgres://user:pass@localhost:5432/db").expect("postgres scheme ok");
    validate_database_path("postgresql://user:pass@localhost/db").expect("postgresql scheme ok");
}

#[test]
fn rejects_empty_url() {
    let err = validate_database_path("").expect_err("empty must error");
    assert!(matches!(err, RuntimeError::EmptyDatabaseUrl));
}

#[test]
fn rejects_missing_sqlite_file() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("does_not_exist.db");
    let err = validate_database_path(&missing.to_string_lossy())
        .expect_err("missing file path must error");
    assert!(matches!(err, RuntimeError::DatabaseNotFound { .. }));
}

#[test]
fn rejects_directory_path() {
    let dir = tempdir().expect("tempdir");
    let err = validate_database_path(&dir.path().to_string_lossy())
        .expect_err("directory path must error");
    assert!(matches!(err, RuntimeError::DatabaseNotFile { .. }));
}

#[test]
fn accepts_existing_sqlite_file() {
    let dir = tempdir().expect("tempdir");
    let file = dir.path().join("test.db");
    fs::write(&file, b"").expect("write empty sqlite stub");
    validate_database_path(&file.to_string_lossy()).expect("existing file path accepted");
}
