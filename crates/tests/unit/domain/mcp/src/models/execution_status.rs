//! Unit tests for McpToolExecutionStatus enum

use systemprompt_mcp::models::McpToolExecutionStatus;

#[test]
fn test_execution_status_pending_as_str() {
    assert_eq!(McpToolExecutionStatus::Pending.as_str(), "pending");
}

#[test]
fn test_execution_status_success_as_str() {
    assert_eq!(McpToolExecutionStatus::Success.as_str(), "success");
}

#[test]
fn test_execution_status_failed_as_str() {
    assert_eq!(McpToolExecutionStatus::Failed.as_str(), "failed");
}

#[test]
fn test_execution_status_from_error_true() {
    assert_eq!(
        McpToolExecutionStatus::from_error(true),
        McpToolExecutionStatus::Failed
    );
}

#[test]
fn test_execution_status_from_error_false() {
    assert_eq!(
        McpToolExecutionStatus::from_error(false),
        McpToolExecutionStatus::Success
    );
}

#[test]
fn test_execution_status_display_pending() {
    assert_eq!(McpToolExecutionStatus::Pending.to_string(), "pending");
}

#[test]
fn test_execution_status_display_success() {
    assert_eq!(McpToolExecutionStatus::Success.to_string(), "success");
}

#[test]
fn test_execution_status_display_failed() {
    assert_eq!(McpToolExecutionStatus::Failed.to_string(), "failed");
}

#[test]
fn timeout_round_trips_and_unknown_values_are_errors() {
    use std::str::FromStr;
    assert_eq!(McpToolExecutionStatus::Timeout.as_str(), "timeout");
    for status in [
        McpToolExecutionStatus::Pending,
        McpToolExecutionStatus::Success,
        McpToolExecutionStatus::Failed,
        McpToolExecutionStatus::Timeout,
    ] {
        assert_eq!(
            McpToolExecutionStatus::from_str(status.as_str()).unwrap(),
            status
        );
    }
    assert_eq!(
        McpToolExecutionStatus::default(),
        McpToolExecutionStatus::Pending
    );
    assert!(McpToolExecutionStatus::from_str("crashed").is_err());
}
