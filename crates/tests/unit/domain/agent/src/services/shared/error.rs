//! Unit tests for AgentServiceError
//!
//! Tests cover:
//! - Error variant creation and display messages
//! - Error conversions from other error types keep the typed cause

use std::error::Error as _;

use systemprompt_agent::services::shared::error::AgentServiceError;

fn parse_err() -> std::num::ParseIntError {
    "not-a-number".parse::<u32>().unwrap_err()
}

#[test]
fn test_agent_service_error_tool_failed_names_tool_and_cause() {
    let error = AgentServiceError::ToolFailed {
        tool_name: systemprompt_identifiers::McpToolName::new("search"),
        message: "Unexpected state".to_string(),
    };
    assert_eq!(error.to_string(), "tool search failed: Unexpected state");
}

#[test]
fn test_agent_service_error_tool_execution() {
    let error = AgentServiceError::ToolExecution("tool x timed out".to_string());
    assert!(error.to_string().contains("Tool execution failed"));
    assert!(error.to_string().contains("tool x timed out"));
}

#[test]
fn operation_keeps_context_and_cause() {
    let error = AgentServiceError::operation("Failed to parse PID from lsof output", parse_err());
    assert!(matches!(error, AgentServiceError::Operation { .. }));
    assert!(error.to_string().contains("Failed to parse PID"));
    assert!(error.source().is_some());
}

#[test]
fn validation_keeps_field_and_cause() {
    let error = AgentServiceError::validation("agent_name", parse_err());
    assert!(matches!(
        error,
        AgentServiceError::Validation {
            field: "agent_name",
            ..
        }
    ));
    assert!(error.source().is_some());
}

#[test]
fn test_result_ok() {
    let result: systemprompt_agent::services::shared::error::Result<i32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_result_err() {
    let result: systemprompt_agent::services::shared::error::Result<i32> =
        Err(AgentServiceError::StreamClosed);
    result.unwrap_err();
}

#[test]
fn from_io_error_maps_to_io() {
    let err: AgentServiceError =
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked").into();
    assert!(matches!(err, AgentServiceError::Io(_)));
    assert!(err.to_string().contains("io: locked"));
}

#[test]
fn from_sqlx_row_not_found_maps_to_repository_not_found() {
    let err: AgentServiceError = sqlx::Error::RowNotFound.into();
    match err {
        AgentServiceError::Repository(inner) => assert!(inner.is_not_found()),
        other => panic!("expected Repository, got {other:?}"),
    }
}

#[test]
fn from_repository_errors_map_to_repository() {
    let err: AgentServiceError =
        systemprompt_agent::repository::RepositoryError::not_found("row", "row gone").into();
    assert!(matches!(err, AgentServiceError::Repository(_)));
    assert!(err.to_string().contains("row gone"));
}

#[test]
fn from_agent_error_maps_to_agent() {
    let err: AgentServiceError =
        systemprompt_agent::AgentError::NotFound("ghost".to_owned()).into();
    assert!(matches!(
        err,
        AgentServiceError::Agent(systemprompt_agent::AgentError::NotFound(_))
    ));
    assert!(err.to_string().contains("ghost"));
}

#[test]
fn from_inference_error_maps_to_ai_inference() {
    let provider_err =
        systemprompt_models::errors::AiInferenceError::InvalidRequest("bad prompt".into());
    let err: AgentServiceError = provider_err.into();
    assert!(matches!(err, AgentServiceError::AiInference(_)));
    assert!(err.to_string().contains("bad prompt"));
}

#[test]
fn from_mcp_registry_error_maps_to_mcp_registry() {
    let registry_err = systemprompt_models::errors::McpRegistryError::NotFound("srv".to_owned());
    let err: AgentServiceError = registry_err.into();
    assert!(matches!(err, AgentServiceError::McpRegistry(_)));
    assert!(err.to_string().contains("srv"));
}
