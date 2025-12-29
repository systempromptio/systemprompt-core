//! Unit tests for ExecutionStatus enum

use systemprompt_core_mcp::models::ExecutionStatus;

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

#[test]
fn test_execution_status_equality() {
    assert_eq!(ExecutionStatus::Pending, ExecutionStatus::Pending);
    assert_eq!(ExecutionStatus::Success, ExecutionStatus::Success);
    assert_eq!(ExecutionStatus::Failed, ExecutionStatus::Failed);
}

#[test]
fn test_execution_status_inequality() {
    assert_ne!(ExecutionStatus::Pending, ExecutionStatus::Success);
    assert_ne!(ExecutionStatus::Success, ExecutionStatus::Failed);
    assert_ne!(ExecutionStatus::Failed, ExecutionStatus::Pending);
}

#[test]
fn test_execution_status_clone() {
    let status = ExecutionStatus::Success;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_execution_status_copy() {
    let status = ExecutionStatus::Pending;
    let copied = status;
    assert_eq!(status, copied);
}

// ============================================================================
// ExecutionStatus Debug Tests
// ============================================================================

#[test]
fn test_execution_status_debug_pending() {
    assert!(format!("{:?}", ExecutionStatus::Pending).contains("Pending"));
}

#[test]
fn test_execution_status_debug_success() {
    assert!(format!("{:?}", ExecutionStatus::Success).contains("Success"));
}

#[test]
fn test_execution_status_debug_failed() {
    assert!(format!("{:?}", ExecutionStatus::Failed).contains("Failed"));
}
