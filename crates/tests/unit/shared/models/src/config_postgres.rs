//! Unit tests for `systemprompt_models::config::validate_postgres_url`.

use systemprompt_models::config::validate_postgres_url;

#[test]
fn valid_postgres_scheme() {
    assert!(validate_postgres_url("postgres://localhost:5432/db").is_ok());
}

#[test]
fn valid_postgresql_scheme() {
    assert!(validate_postgres_url("postgresql://user:pass@host:5432/db").is_ok());
}

#[test]
fn rejects_mysql_scheme() {
    assert!(validate_postgres_url("mysql://localhost/db").is_err());
}

#[test]
fn rejects_empty() {
    assert!(validate_postgres_url("").is_err());
}

#[test]
fn rejects_file_path() {
    assert!(validate_postgres_url("/var/data/database.db").is_err());
}
