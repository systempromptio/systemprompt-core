//! Tests for AgentExtension, Task, TaskStatus, and TaskState.

use systemprompt_models::errors::ParseEnumError;
use systemprompt_models::{AgentExtension, Task, TaskState, TaskStatus};

#[test]
fn test_agent_extension_mcp_tools() {
    let ext = AgentExtension::mcp_tools_extension();

    assert_eq!(ext.uri, "systemprompt:mcp-tools");
    ext.description.as_ref().expect("description should be set");
    assert_eq!(ext.required, Some(false));
    ext.params.as_ref().expect("params should be set");
}

#[test]
fn test_agent_extension_agent_identity() {
    let ext = AgentExtension::agent_identity(&systemprompt_identifiers::AgentName::new("my-agent"));

    assert_eq!(ext.uri, "systemprompt:agent-identity");
    assert_eq!(ext.required, Some(true));
    let params = ext.params.unwrap();
    assert_eq!(params.get("name").unwrap(), "my-agent");
}

#[test]
fn test_agent_extension_system_instructions() {
    let ext = AgentExtension::system_instructions("You are a helpful assistant");

    assert_eq!(ext.uri, "systemprompt:system-instructions");
    assert_eq!(ext.required, Some(true));
    let params = ext.params.unwrap();
    assert_eq!(
        params.get("systemPrompt").unwrap(),
        "You are a helpful assistant"
    );
}

#[test]
fn test_agent_extension_system_instructions_opt_some() {
    let ext = AgentExtension::system_instructions_opt(Some("test prompt"));
    ext.expect("system_instructions_opt with Some should return Some");
}

#[test]
fn test_agent_extension_system_instructions_opt_none() {
    let ext = AgentExtension::system_instructions_opt(None);
    assert!(ext.is_none());
}

#[test]
fn test_agent_extension_service_status() {
    let ext = AgentExtension::service_status("running", Some(8080), Some(1234), true);

    assert_eq!(ext.uri, "systemprompt:service-status");
    let params = ext.params.unwrap();
    assert_eq!(params.get("status").unwrap(), "running");
    assert_eq!(params.get("port").unwrap(), 8080);
    assert_eq!(params.get("pid").unwrap(), 1234);
    assert_eq!(params.get("default").unwrap(), true);
}

#[test]
fn test_agent_extension_service_status_without_optional() {
    let ext = AgentExtension::service_status("stopped", None, None, false);

    let params = ext.params.unwrap();
    assert!(params.get("port").is_none());
    assert!(params.get("pid").is_none());
}

#[test]
fn test_task_default() {
    let task = Task::default();

    assert!(task.history.is_none());
    assert!(task.artifacts.is_none());
    assert!(task.metadata.is_none());
    assert!(matches!(task.status.state, TaskState::Submitted));
}

#[test]
fn test_task_status_default() {
    let status = TaskStatus::default();

    assert!(matches!(status.state, TaskState::Submitted));
    assert!(status.message.is_none());
    assert!(status.timestamp.is_none());
}

#[test]
fn test_task_state_submitted() {
    let state: TaskState = "TASK_STATE_SUBMITTED".parse().unwrap();
    assert_eq!(state, TaskState::Submitted);
    assert_eq!(state.as_str(), "TASK_STATE_SUBMITTED");
}

#[test]
fn test_task_state_working() {
    let state: TaskState = "TASK_STATE_WORKING".parse().unwrap();
    assert_eq!(state, TaskState::Working);
    assert_eq!(state.as_str(), "TASK_STATE_WORKING");
}

#[test]
fn test_task_state_completed() {
    let state: TaskState = "TASK_STATE_COMPLETED".parse().unwrap();
    assert_eq!(state, TaskState::Completed);
    assert_eq!(state.as_str(), "TASK_STATE_COMPLETED");
}

#[test]
fn test_task_state_failed() {
    let state: TaskState = "TASK_STATE_FAILED".parse().unwrap();
    assert_eq!(state, TaskState::Failed);
    assert_eq!(state.as_str(), "TASK_STATE_FAILED");
}

#[test]
fn test_task_state_canceled() {
    let state: TaskState = "TASK_STATE_CANCELED".parse().unwrap();
    assert_eq!(state, TaskState::Canceled);
    assert_eq!(state.as_str(), "TASK_STATE_CANCELED");
}

#[test]
fn test_task_state_rejected() {
    let state: TaskState = "TASK_STATE_REJECTED".parse().unwrap();
    assert_eq!(state, TaskState::Rejected);
    assert_eq!(state.as_str(), "TASK_STATE_REJECTED");
}

#[test]
fn test_task_state_input_required() {
    let state: TaskState = "TASK_STATE_INPUT_REQUIRED".parse().unwrap();
    assert_eq!(state, TaskState::InputRequired);
    assert_eq!(state.as_str(), "TASK_STATE_INPUT_REQUIRED");
}

#[test]
fn test_task_state_auth_required() {
    let state: TaskState = "TASK_STATE_AUTH_REQUIRED".parse().unwrap();
    assert_eq!(state, TaskState::AuthRequired);
    assert_eq!(state.as_str(), "TASK_STATE_AUTH_REQUIRED");
}

#[test]
fn test_task_state_rejects_unknown_names() {
    for legacy in ["cancelled", "Working", "pending", "unknown"] {
        let err = legacy.parse::<TaskState>().unwrap_err();
        assert_eq!(err.value, legacy);
    }
}

#[test]
fn test_task_state_rejects_removed_wire_values() {
    for removed in ["TASK_STATE_PENDING", "TASK_STATE_UNKNOWN"] {
        assert!(removed.parse::<TaskState>().is_err());
        assert!(serde_json::from_value::<TaskState>(serde_json::json!(removed)).is_err());
    }
}

#[test]
fn test_task_state_invalid() {
    let result: Result<TaskState, ParseEnumError> = "invalid-state".parse();
    let err = result.unwrap_err();
    assert_eq!(err.value, "invalid-state");
}
