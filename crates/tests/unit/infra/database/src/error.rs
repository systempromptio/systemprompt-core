//! Unit tests for RepositoryError

use std::error::Error;

use systemprompt_traits::RepositoryError;

#[test]
fn test_not_found_from_string() {
    let error = RepositoryError::not_found("user", "user-123");
    assert!(matches!(
        error,
        RepositoryError::NotFound { entity: "user", key: Some(ref key) } if key == "user-123"
    ));
    assert!(error.to_string().contains("user-123"));
}

#[test]
fn test_not_found_from_integer() {
    let error = RepositoryError::not_found("row", 42);
    assert!(matches!(error, RepositoryError::NotFound { .. }));
    assert!(error.to_string().contains("42"));
}

#[test]
fn test_conflict_from_string() {
    let error = RepositoryError::conflict("task", "t1", "stale task update");
    assert!(matches!(
        error,
        RepositoryError::Conflict { entity: "task", ref key, .. } if key == "t1"
    ));
    assert!(error.to_string().contains("stale task update"));
}

#[test]
fn test_conflict_is_a_conflict_but_not_a_constraint() {
    let error = RepositoryError::conflict("task", "t1", String::from("invalid transition"));
    assert!(error.is_conflict());
    assert!(!error.is_constraint());
}

#[test]
fn test_row_not_found_classifies_as_not_found() {
    let error = RepositoryError::from(sqlx::Error::RowNotFound);
    assert!(error.is_not_found());
    assert_eq!(error.to_string(), "row not found");
}

#[test]
fn test_non_database_sqlx_error_keeps_its_source() {
    let error = RepositoryError::from(sqlx::Error::PoolClosed);
    assert!(matches!(
        error,
        RepositoryError::Database { sqlstate: None, .. }
    ));
    let source = error.source().expect("database error keeps its source");
    assert!(source.downcast_ref::<sqlx::Error>().is_some());
}

#[test]
fn test_database_constructor_classifies_a_sqlx_error() {
    let error = RepositoryError::database(sqlx::Error::RowNotFound);
    assert!(error.is_not_found());
}

#[test]
fn test_database_constructor_boxes_a_foreign_error() {
    let error = RepositoryError::database(std::io::Error::other("disk gone"));
    assert!(matches!(error, RepositoryError::Database { .. }));
    assert!(error.to_string().contains("disk gone"));
    assert!(!error.is_serialization_failure());
}

#[test]
fn test_invalid_argument_from_str() {
    let error = RepositoryError::invalid_argument("email", "cannot be empty");
    assert!(matches!(
        error,
        RepositoryError::InvalidArgument { field: "email", .. }
    ));
    assert_eq!(error.to_string(), "invalid argument email: cannot be empty");
}

#[test]
fn test_invalid_argument_from_owned_string() {
    let error = RepositoryError::invalid_argument("email", String::from("invalid format"));
    assert!(matches!(error, RepositoryError::InvalidArgument { .. }));
    assert!(error.to_string().contains("invalid format"));
}

#[test]
fn test_internal_from_str() {
    let error = RepositoryError::internal("unexpected state");
    assert!(matches!(error, RepositoryError::Internal(_)));
    assert!(error.to_string().contains("unexpected state"));
}

#[test]
fn test_internal_from_owned_string() {
    let error = RepositoryError::internal(String::from("connection pool exhausted"));
    assert!(matches!(error, RepositoryError::Internal(_)));
    assert!(error.to_string().contains("connection pool exhausted"));
}

#[test]
fn test_is_not_found_returns_true_for_not_found() {
    let error = RepositoryError::not_found("row", "id");
    assert!(error.is_not_found());
}

#[test]
fn test_is_not_found_returns_false_for_conflict() {
    let error = RepositoryError::conflict("row", "r1", "violation");
    assert!(!error.is_not_found());
}

#[test]
fn test_is_not_found_returns_false_for_invalid_argument() {
    let error = RepositoryError::invalid_argument("input", "bad input");
    assert!(!error.is_not_found());
}

#[test]
fn test_is_not_found_returns_false_for_internal() {
    let error = RepositoryError::internal("oops");
    assert!(!error.is_not_found());
}

#[test]
fn test_is_constraint_returns_false_for_not_found() {
    let error = RepositoryError::not_found("row", "id");
    assert!(!error.is_constraint());
}

#[test]
fn test_is_constraint_returns_false_for_invalid_argument() {
    let error = RepositoryError::invalid_argument("input", "bad");
    assert!(!error.is_constraint());
}

#[test]
fn test_is_constraint_returns_false_for_internal() {
    let error = RepositoryError::internal("error");
    assert!(!error.is_constraint());
}

#[test]
fn test_not_found_display() {
    let error = RepositoryError::not_found("user", "user-456");
    assert_eq!(error.to_string(), "user not found: user-456");
}

#[test]
fn test_conflict_display() {
    let error = RepositoryError::conflict("user", "u1", "duplicate key");
    let display = error.to_string();
    assert!(
        display.contains("conflict") && display.contains("duplicate key"),
        "Expected display to name the conflict, got: {}",
        display
    );
}

#[test]
fn test_invalid_argument_display() {
    let error = RepositoryError::invalid_argument("name", "missing field");
    let display = error.to_string();
    assert!(
        display.contains("Invalid") || display.contains("invalid"),
        "Expected display to contain 'invalid', got: {}",
        display
    );
}

#[test]
fn test_internal_display() {
    let error = RepositoryError::internal("system failure");
    let display = error.to_string();
    assert!(
        display.contains("Internal") || display.contains("internal"),
        "Expected display to contain 'internal', got: {}",
        display
    );
}

#[test]
fn test_from_serde_json_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("not valid json").unwrap_err();
    let repo_err: RepositoryError = json_err.into();
    assert!(matches!(repo_err, RepositoryError::Serialization(_)));
}
