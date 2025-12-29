//! Unit tests for agent error types
//!
//! Tests cover:
//! - TaskError variants and error messages
//! - ContextError variants and error messages
//! - ArtifactError variants and error messages
//! - ProtocolError variants and error messages
//! - AgentError conversions and wrapping

use systemprompt_core_agent::{AgentError, ArtifactError, ContextError, ProtocolError, TaskError};

// ============================================================================
// TaskError Tests
// ============================================================================

#[test]
fn test_task_error_missing_task_uuid_display() {
    let error = TaskError::MissingTaskUuid;
    assert_eq!(error.to_string(), "Task UUID missing from database row");
}

#[test]
fn test_task_error_missing_agent_name_display() {
    let error = TaskError::MissingAgentName {
        task_id: "task-123".to_string(),
    };
    assert!(error.to_string().contains("task-123"));
    assert!(error.to_string().contains("Agent name not found"));
}

#[test]
fn test_task_error_missing_context_id_display() {
    let error = TaskError::MissingContextId;
    assert_eq!(error.to_string(), "Context ID missing from database row");
}

#[test]
fn test_task_error_invalid_task_state_display() {
    let error = TaskError::InvalidTaskState {
        state: "invalid_state".to_string(),
    };
    assert!(error.to_string().contains("invalid_state"));
    assert!(error.to_string().contains("Invalid task state"));
}

#[test]
fn test_task_error_missing_field_display() {
    let error = TaskError::MissingField {
        field: "context_id".to_string(),
    };
    assert!(error.to_string().contains("context_id"));
    assert!(error.to_string().contains("Missing required field"));
}

#[test]
fn test_task_error_invalid_datetime_display() {
    let error = TaskError::InvalidDatetime {
        field: "created_at".to_string(),
    };
    assert!(error.to_string().contains("created_at"));
    assert!(error.to_string().contains("Invalid datetime"));
}

#[test]
fn test_task_error_empty_task_id_display() {
    let error = TaskError::EmptyTaskId;
    assert_eq!(error.to_string(), "Empty task ID provided");
}

#[test]
fn test_task_error_invalid_task_id_format_display() {
    let error = TaskError::InvalidTaskIdFormat {
        id: "not-a-uuid".to_string(),
    };
    assert!(error.to_string().contains("not-a-uuid"));
    assert!(error.to_string().contains("Invalid task ID format"));
}

#[test]
fn test_task_error_missing_message_id_display() {
    let error = TaskError::MissingMessageId;
    assert_eq!(error.to_string(), "Message ID missing from database row");
}

#[test]
fn test_task_error_missing_tool_name_display() {
    let error = TaskError::MissingToolName;
    assert_eq!(error.to_string(), "Tool name missing for tool execution");
}

#[test]
fn test_task_error_missing_call_id_display() {
    let error = TaskError::MissingCallId;
    assert_eq!(error.to_string(), "Tool call ID missing for tool execution");
}

#[test]
fn test_task_error_missing_created_timestamp_display() {
    let error = TaskError::MissingCreatedTimestamp;
    assert_eq!(error.to_string(), "Created timestamp missing from database");
}

// ============================================================================
// ContextError Tests
// ============================================================================

#[test]
fn test_context_error_missing_uuid_display() {
    let error = ContextError::MissingUuid;
    assert_eq!(error.to_string(), "Context UUID missing from database row");
}

#[test]
fn test_context_error_missing_name_display() {
    let error = ContextError::MissingName;
    assert_eq!(error.to_string(), "Context name missing from database row");
}

#[test]
fn test_context_error_missing_user_id_display() {
    let error = ContextError::MissingUserId;
    assert_eq!(error.to_string(), "User ID missing from database row");
}

#[test]
fn test_context_error_missing_field_display() {
    let error = ContextError::MissingField {
        field: "description".to_string(),
    };
    assert!(error.to_string().contains("description"));
    assert!(error.to_string().contains("Missing required field"));
}

#[test]
fn test_context_error_invalid_datetime_display() {
    let error = ContextError::InvalidDatetime {
        field: "updated_at".to_string(),
    };
    assert!(error.to_string().contains("updated_at"));
    assert!(error.to_string().contains("Invalid datetime"));
}

// ============================================================================
// ArtifactError Tests
// ============================================================================

#[test]
fn test_artifact_error_missing_uuid_display() {
    let error = ArtifactError::MissingUuid;
    assert_eq!(error.to_string(), "Artifact UUID missing from database row");
}

#[test]
fn test_artifact_error_missing_type_display() {
    let error = ArtifactError::MissingType;
    assert_eq!(error.to_string(), "Artifact type missing from database row");
}

#[test]
fn test_artifact_error_missing_context_id_display() {
    let error = ArtifactError::MissingContextId;
    assert_eq!(error.to_string(), "Context ID missing for artifact");
}

#[test]
fn test_artifact_error_missing_field_display() {
    let error = ArtifactError::MissingField {
        field: "content".to_string(),
    };
    assert!(error.to_string().contains("content"));
    assert!(error.to_string().contains("Missing required field"));
}

#[test]
fn test_artifact_error_invalid_datetime_display() {
    let error = ArtifactError::InvalidDatetime {
        field: "created_at".to_string(),
    };
    assert!(error.to_string().contains("created_at"));
    assert!(error.to_string().contains("Invalid datetime"));
}

#[test]
fn test_artifact_error_transform_display() {
    let error = ArtifactError::Transform("Failed to convert format".to_string());
    assert!(error.to_string().contains("Transform error"));
    assert!(error.to_string().contains("Failed to convert format"));
}

#[test]
fn test_artifact_error_metadata_validation_display() {
    let error = ArtifactError::MetadataValidation("Invalid schema".to_string());
    assert!(error.to_string().contains("Metadata validation error"));
    assert!(error.to_string().contains("Invalid schema"));
}

// ============================================================================
// ProtocolError Tests
// ============================================================================

#[test]
fn test_protocol_error_missing_tool_name_display() {
    let error = ProtocolError::MissingToolName;
    assert_eq!(error.to_string(), "Tool name missing in tool call");
}

#[test]
fn test_protocol_error_missing_error_flag_display() {
    let error = ProtocolError::MissingErrorFlag;
    assert!(error.to_string().contains("error flag"));
}

#[test]
fn test_protocol_error_missing_message_id_display() {
    let error = ProtocolError::MissingMessageId;
    assert_eq!(error.to_string(), "Message ID missing");
}

#[test]
fn test_protocol_error_missing_request_id_display() {
    let error = ProtocolError::MissingRequestId;
    assert_eq!(error.to_string(), "Request ID missing");
}

#[test]
fn test_protocol_error_invalid_latency_display() {
    let error = ProtocolError::InvalidLatency;
    assert!(error.to_string().contains("Latency"));
}

#[test]
fn test_protocol_error_validation_failed_display() {
    let error = ProtocolError::ValidationFailed("Invalid message format".to_string());
    assert!(error.to_string().contains("Validation failed"));
    assert!(error.to_string().contains("Invalid message format"));
}

// ============================================================================
// AgentError Tests
// ============================================================================

#[test]
fn test_agent_error_from_task_error() {
    let task_error = TaskError::EmptyTaskId;
    let agent_error: AgentError = task_error.into();

    match agent_error {
        AgentError::Task(_) => {}
        _ => panic!("Expected AgentError::Task variant"),
    }
}

#[test]
fn test_agent_error_from_context_error() {
    let context_error = ContextError::MissingUuid;
    let agent_error: AgentError = context_error.into();

    match agent_error {
        AgentError::Context(_) => {}
        _ => panic!("Expected AgentError::Context variant"),
    }
}

#[test]
fn test_agent_error_from_artifact_error() {
    let artifact_error = ArtifactError::MissingUuid;
    let agent_error: AgentError = artifact_error.into();

    match agent_error {
        AgentError::Artifact(_) => {}
        _ => panic!("Expected AgentError::Artifact variant"),
    }
}

#[test]
fn test_agent_error_from_protocol_error() {
    let protocol_error = ProtocolError::MissingMessageId;
    let agent_error: AgentError = protocol_error.into();

    match agent_error {
        AgentError::Protocol(_) => {}
        _ => panic!("Expected AgentError::Protocol variant"),
    }
}

#[test]
fn test_agent_error_database_display() {
    let error = AgentError::Database("Connection failed".to_string());
    assert!(error.to_string().contains("Database error"));
    assert!(error.to_string().contains("Connection failed"));
}

#[test]
fn test_agent_error_task_display() {
    let task_error = TaskError::EmptyTaskId;
    let agent_error: AgentError = task_error.into();
    assert!(agent_error.to_string().contains("Task error"));
}

#[test]
fn test_agent_error_context_display() {
    let context_error = ContextError::MissingName;
    let agent_error: AgentError = context_error.into();
    assert!(agent_error.to_string().contains("Context error"));
}

#[test]
fn test_agent_error_artifact_display() {
    let artifact_error = ArtifactError::MissingType;
    let agent_error: AgentError = artifact_error.into();
    assert!(agent_error.to_string().contains("Artifact error"));
}

#[test]
fn test_agent_error_protocol_display() {
    let protocol_error = ProtocolError::MissingRequestId;
    let agent_error: AgentError = protocol_error.into();
    assert!(agent_error.to_string().contains("A2A protocol error"));
}

// ============================================================================
// Error Debug Implementation Tests
// ============================================================================

#[test]
fn test_task_error_debug() {
    let error = TaskError::MissingTaskUuid;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("MissingTaskUuid"));
}

#[test]
fn test_context_error_debug() {
    let error = ContextError::MissingUuid;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("MissingUuid"));
}

#[test]
fn test_artifact_error_debug() {
    let error = ArtifactError::MissingUuid;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("MissingUuid"));
}

#[test]
fn test_protocol_error_debug() {
    let error = ProtocolError::MissingToolName;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("MissingToolName"));
}

#[test]
fn test_agent_error_debug() {
    let error = AgentError::Database("test".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("Database"));
}
