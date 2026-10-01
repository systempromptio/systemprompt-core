use systemprompt_runtime::{RuntimeError, validate_database_url};

#[test]
fn test_empty_url_returns_error() {
    let result = validate_database_url("");
    assert!(matches!(result, Err(RuntimeError::EmptyDatabaseUrl)));
}

#[test]
fn test_postgresql_url_accepted() {
    validate_database_url("postgresql://localhost:5432/testdb").expect("postgresql url accepted");
}

#[test]
fn test_postgres_url_accepted() {
    validate_database_url("postgres://localhost:5432/testdb").expect("postgres url accepted");
}

#[test]
fn test_postgresql_url_with_credentials() {
    validate_database_url("postgresql://user:pass@localhost:5432/testdb")
        .expect("postgresql url with credentials accepted");
}

#[test]
fn test_postgres_url_with_ssl_options() {
    validate_database_url("postgres://localhost:5432/testdb?sslmode=require")
        .expect("postgres url with ssl options accepted");
}

#[test]
fn test_file_path_is_unsupported() {
    let result = validate_database_url("/var/lib/app/database.db");
    assert!(matches!(result, Err(RuntimeError::UnsupportedDatabaseUrl)));
}

#[test]
fn test_mysql_url_is_unsupported() {
    let result = validate_database_url("mysql://localhost/db");
    assert!(matches!(result, Err(RuntimeError::UnsupportedDatabaseUrl)));
}

#[test]
fn test_http_url_is_unsupported() {
    let result = validate_database_url("http://localhost/db");
    assert!(matches!(result, Err(RuntimeError::UnsupportedDatabaseUrl)));
}

#[test]
fn test_whitespace_only_url_is_unsupported_not_empty() {
    let result = validate_database_url("   ");
    assert!(matches!(result, Err(RuntimeError::UnsupportedDatabaseUrl)));
}

#[test]
fn test_postgresql_prefix_case_sensitive() {
    let result = validate_database_url("PostgreSQL://localhost/db");
    assert!(result.is_err());
}

#[test]
fn test_postgres_url_no_port() {
    validate_database_url("postgres://localhost/db").expect("postgres url without port accepted");
}

#[test]
fn test_postgres_url_with_at_sign_in_password() {
    validate_database_url("postgres://user:p%40ss@localhost:5432/db")
        .expect("postgres url with at-sign in password accepted");
}

#[test]
fn test_unsupported_url_message_does_not_echo_the_url() {
    let err = validate_database_url("mysql://admin:hunter2@db/app").expect_err("unsupported");
    assert!(!err.to_string().contains("hunter2"), "{err}");
}
