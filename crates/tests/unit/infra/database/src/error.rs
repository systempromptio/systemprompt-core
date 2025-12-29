//! Unit tests for RepositoryError

use systemprompt_core_database::RepositoryError;

// ============================================================================
// RepositoryError Construction Tests
// ============================================================================

#[test]
fn test_not_found_from_string() {
    let error = RepositoryError::not_found("user-123");
    assert!(matches!(error, RepositoryError::NotFound(_)));
    assert!(error.to_string().contains("user-123"));
}

#[test]
fn test_not_found_from_integer() {
    let error = RepositoryError::not_found(42);
    assert!(matches!(error, RepositoryError::NotFound(_)));
    assert!(error.to_string().contains("42"));
}

#[test]
fn test_constraint_from_string() {
    let error = RepositoryError::constraint("unique_email");
    assert!(matches!(error, RepositoryError::Constraint(_)));
    assert!(error.to_string().contains("unique_email"));
}

#[test]
fn test_constraint_from_owned_string() {
    let error = RepositoryError::constraint(String::from("foreign_key_violation"));
    assert!(matches!(error, RepositoryError::Constraint(_)));
    assert!(error.to_string().contains("foreign_key_violation"));
}

#[test]
fn test_invalid_argument_from_str() {
    let error = RepositoryError::invalid_argument("email cannot be empty");
    assert!(matches!(error, RepositoryError::InvalidArgument(_)));
    assert!(error.to_string().contains("email cannot be empty"));
}

#[test]
fn test_invalid_argument_from_owned_string() {
    let error = RepositoryError::invalid_argument(String::from("invalid format"));
    assert!(matches!(error, RepositoryError::InvalidArgument(_)));
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

// ============================================================================
// RepositoryError Predicate Tests
// ============================================================================

#[test]
fn test_is_not_found_returns_true_for_not_found() {
    let error = RepositoryError::not_found("id");
    assert!(error.is_not_found());
}

#[test]
fn test_is_not_found_returns_false_for_constraint() {
    let error = RepositoryError::constraint("violation");
    assert!(!error.is_not_found());
}

#[test]
fn test_is_not_found_returns_false_for_invalid_argument() {
    let error = RepositoryError::invalid_argument("bad input");
    assert!(!error.is_not_found());
}

#[test]
fn test_is_not_found_returns_false_for_internal() {
    let error = RepositoryError::internal("oops");
    assert!(!error.is_not_found());
}

#[test]
fn test_is_constraint_returns_true_for_constraint() {
    let error = RepositoryError::constraint("unique");
    assert!(error.is_constraint());
}

#[test]
fn test_is_constraint_returns_false_for_not_found() {
    let error = RepositoryError::not_found("id");
    assert!(!error.is_constraint());
}

#[test]
fn test_is_constraint_returns_false_for_invalid_argument() {
    let error = RepositoryError::invalid_argument("bad");
    assert!(!error.is_constraint());
}

#[test]
fn test_is_constraint_returns_false_for_internal() {
    let error = RepositoryError::internal("error");
    assert!(!error.is_constraint());
}

// ============================================================================
// RepositoryError Display Tests
// ============================================================================

#[test]
fn test_not_found_display() {
    let error = RepositoryError::not_found("user-456");
    let display = error.to_string();
    assert!(display.contains("not found") || display.contains("Not found"));
    assert!(display.contains("user-456"));
}

#[test]
fn test_constraint_display() {
    let error = RepositoryError::constraint("duplicate key");
    let display = error.to_string();
    assert!(
        display.contains("Constraint") || display.contains("constraint"),
        "Expected display to contain 'constraint', got: {}",
        display
    );
}

#[test]
fn test_invalid_argument_display() {
    let error = RepositoryError::invalid_argument("missing field");
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

// ============================================================================
// RepositoryError From Implementations Tests
// ============================================================================

#[test]
fn test_from_anyhow_error() {
    let anyhow_err = anyhow::anyhow!("something went wrong");
    let repo_err: RepositoryError = anyhow_err.into();
    assert!(matches!(repo_err, RepositoryError::Internal(_)));
    assert!(repo_err.to_string().contains("something went wrong"));
}

#[test]
fn test_from_serde_json_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("not valid json").unwrap_err();
    let repo_err: RepositoryError = json_err.into();
    assert!(matches!(repo_err, RepositoryError::Serialization(_)));
}

// ============================================================================
// RepositoryError Debug Tests
// ============================================================================

#[test]
fn test_debug_format() {
    let error = RepositoryError::not_found("test-id");
    let debug = format!("{:?}", error);
    assert!(debug.contains("NotFound"));
}
