use systemprompt_runtime::{RuntimeError, validate_database_url};

#[test]
fn accepts_postgres_url() {
    validate_database_url("postgres://user:pass@localhost:5432/db").expect("postgres scheme ok");
    validate_database_url("postgresql://user:pass@localhost/db").expect("postgresql scheme ok");
}

#[test]
fn rejects_empty_url() {
    let err = validate_database_url("").expect_err("empty must error");
    assert!(matches!(err, RuntimeError::EmptyDatabaseUrl));
}

#[test]
fn rejects_a_file_path() {
    let err = validate_database_url("/tmp/app.db").expect_err("a file path is not a database URL");
    assert!(matches!(err, RuntimeError::UnsupportedDatabaseUrl));
}
