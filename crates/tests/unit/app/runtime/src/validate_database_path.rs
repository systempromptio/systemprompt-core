use systemprompt_runtime::validate_database_path;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_empty_path_returns_error() {
    let result = validate_database_path("");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("empty"));
}

#[test]
fn test_postgresql_url_accepted() {
    let result = validate_database_path("postgresql://localhost:5432/testdb");
    assert!(result.is_ok());
}

#[test]
fn test_postgres_url_accepted() {
    let result = validate_database_path("postgres://localhost:5432/testdb");
    assert!(result.is_ok());
}

#[test]
fn test_postgresql_url_with_credentials() {
    let result = validate_database_path("postgresql://user:pass@localhost:5432/testdb");
    assert!(result.is_ok());
}

#[test]
fn test_postgres_url_with_ssl_options() {
    let result = validate_database_path("postgres://localhost:5432/testdb?sslmode=require");
    assert!(result.is_ok());
}

#[test]
fn test_nonexistent_file_path_returns_error() {
    let result = validate_database_path("/nonexistent/path/to/database.db");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not found"));
}

#[test]
fn test_directory_path_returns_not_a_file_error() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_str().unwrap();
    let result = validate_database_path(dir_path);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not a file"));
}

#[test]
fn test_existing_file_path_accepted() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_path = temp_file.path().to_str().unwrap();
    let result = validate_database_path(file_path);
    assert!(result.is_ok());
}

#[test]
fn test_mysql_url_treated_as_file_path() {
    let result = validate_database_path("mysql://localhost/db");
    assert!(result.is_err());
}

#[test]
fn test_http_url_treated_as_file_path() {
    let result = validate_database_path("http://localhost/db");
    assert!(result.is_err());
}

#[test]
fn test_whitespace_only_path_not_treated_as_empty() {
    let result = validate_database_path("   ");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(!err_msg.contains("empty"));
}

#[test]
fn test_postgresql_prefix_case_sensitive() {
    let result = validate_database_path("PostgreSQL://localhost/db");
    assert!(result.is_err());
}

#[test]
fn test_postgres_url_no_port() {
    let result = validate_database_path("postgres://localhost/db");
    assert!(result.is_ok());
}

#[test]
fn test_postgres_url_with_at_sign_in_password() {
    let result = validate_database_path("postgres://user:p%40ss@localhost:5432/db");
    assert!(result.is_ok());
}
