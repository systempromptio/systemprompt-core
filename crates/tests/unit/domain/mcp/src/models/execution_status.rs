//! Unit tests for ExecutionStatus enum

use systemprompt_mcp::models::ExecutionStatus;

// ============================================================================
// ExecutionStatus as_str Tests
// ============================================================================

#[test]
fn test_execution_status_pending_as_str() {
    assert_eq!(ExecutionStatus::Pending.as_str(), "pending");
}

#[test]
fn test_execution_status_success_as_str() {
    assert_eq!(ExecutionStatus::Success.as_str(), "success");
}

#[test]
fn test_execution_status_failed_as_str() {
    assert_eq!(ExecutionStatus::Failed.as_str(), "failed");
}

// ============================================================================
// ExecutionStatus from_error Tests
// ============================================================================

#[test]
fn test_execution_status_from_error_true() {
    assert_eq!(ExecutionStatus::from_error(true), ExecutionStatus::Failed);
}

#[test]
fn test_execution_status_from_error_false() {
    assert_eq!(ExecutionStatus::from_error(false), ExecutionStatus::Success);
}

// ============================================================================
// ExecutionStatus Display Tests
// ============================================================================

#[test]
fn test_execution_status_display_pending() {
    assert_eq!(ExecutionStatus::Pending.to_string(), "pending");
}

#[test]
fn test_execution_status_display_success() {
    assert_eq!(ExecutionStatus::Success.to_string(), "success");
}

#[test]
fn test_execution_status_display_failed() {
    assert_eq!(ExecutionStatus::Failed.to_string(), "failed");
}

// ============================================================================
// ExecutionStatus Equality and Clone Tests
// ============================================================================

