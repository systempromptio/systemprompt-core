//! Tests for AgentExtension, AgentSkill, Task, TaskStatus, and TaskState.

use systemprompt_models::{AgentExtension, AgentSkill, Task, TaskState, TaskStatus};

// ============================================================================
// AgentExtension Tests
// ============================================================================

#[test]
fn test_agent_extension_mcp_tools() {
    let ext = AgentExtension::mcp_tools_extension();

    assert_eq!(ext.uri, "systemprompt:mcp-tools");
    ext.description.as_ref().expect("description should be set");
    assert_eq!(ext.required, Some(false));
    ext.params.as_ref().expect("params should be set");
}

#[test]
fn test_agent_extension_mcp_tools_with_servers() {
    let servers = vec![serde_json::json!({"name": "test-server"})];
    let ext = AgentExtension::mcp_tools_extension_with_servers(&servers);

    assert_eq!(ext.uri, "systemprompt:mcp-tools");
    let params = ext.params.expect("params should be set");
    params.get("servers").expect("servers param should be set");
}

#[test]
fn test_agent_extension_opencode_integration() {
    let ext = AgentExtension::opencode_integration_extension();

    assert_eq!(ext.uri, "systemprompt:opencode-integration");
    ext.description.as_ref().expect("description should be set");
}

#[test]
fn test_agent_extension_artifact_rendering() {
    let ext = AgentExtension::artifact_rendering_extension();

    assert!(ext.uri.contains("artifact-rendering"));
    let params = ext.params.expect("params should be set");
    params
        .get("supported_types")
        .expect("supported_types param should be set");
}

#[test]
fn test_agent_extension_agent_identity() {
    let ext = AgentExtension::agent_identity("my-agent");

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

// ============================================================================
// AgentSkill Tests
// ============================================================================

#[test]
fn test_agent_skill_from_mcp_server() {
    let skill = AgentSkill::from_mcp_server(
        "my-server".to_string(),
        "My Server".to_string(),
        "A test server".to_string(),
        vec!["tag1".to_string(), "tag2".to_string()],
    );

    assert_eq!(skill.id, "my-server");
    assert_eq!(skill.name, "My Server");
    assert_eq!(skill.description, "A test server");
    assert_eq!(skill.tags.len(), 2);
    assert!(skill.examples.is_none());
    assert!(skill.input_modes.is_none());
    assert!(skill.output_modes.is_none());
    assert!(skill.security.is_none());
}

#[test]
fn test_agent_skill_mcp_server_name() {
    let skill = AgentSkill::from_mcp_server(
        "test-server".to_string(),
        "Test".to_string(),
        "Desc".to_string(),
        vec![],
    );

    assert_eq!(skill.mcp_server_name(), "test-server");
}

// ============================================================================
// Task Tests
// ============================================================================

#[test]
fn test_task_default() {
    let task = Task::default();

    assert!(task.history.is_none());
    assert!(task.artifacts.is_none());
    assert!(task.metadata.is_none());
    assert!(matches!(task.status.state, TaskState::Submitted));
}

// ============================================================================
// TaskStatus Tests
// ============================================================================

#[test]
fn test_task_status_default() {
    let status = TaskStatus::default();

    assert!(matches!(status.state, TaskState::Submitted));
    assert!(status.message.is_none());
    assert!(status.timestamp.is_none());
}

// ============================================================================
// TaskState Tests
// ============================================================================

#[test]
fn test_task_state_pending() {
    let state: TaskState = "pending".parse().unwrap();
    assert!(matches!(state, TaskState::Pending));
}

#[test]
fn test_task_state_submitted() {
    let state: TaskState = "submitted".parse().unwrap();
    assert!(matches!(state, TaskState::Submitted));
}

#[test]
fn test_task_state_working() {
    let state: TaskState = "working".parse().unwrap();
    assert!(matches!(state, TaskState::Working));
}

#[test]
fn test_task_state_completed() {
    let state: TaskState = "completed".parse().unwrap();
    assert!(matches!(state, TaskState::Completed));
}

#[test]
fn test_task_state_failed() {
    let state: TaskState = "failed".parse().unwrap();
    assert!(matches!(state, TaskState::Failed));
}

#[test]
fn test_task_state_canceled() {
    let state: TaskState = "canceled".parse().unwrap();
    assert!(matches!(state, TaskState::Canceled));
}

#[test]
fn test_task_state_rejected() {
    let state: TaskState = "rejected".parse().unwrap();
    assert!(matches!(state, TaskState::Rejected));
}

#[test]
fn test_task_state_input_required() {
    let state: TaskState = "input-required".parse().unwrap();
    assert!(matches!(state, TaskState::InputRequired));
}

#[test]
fn test_task_state_auth_required() {
    let state: TaskState = "auth-required".parse().unwrap();
    assert!(matches!(state, TaskState::AuthRequired));
}

#[test]
fn test_task_state_unknown() {
    let state: TaskState = "unknown".parse().unwrap();
    assert!(matches!(state, TaskState::Unknown));
}

#[test]
fn test_task_state_invalid() {
    let result: Result<TaskState, String> = "invalid-state".parse();
    let err = result.unwrap_err();
    assert!(err.contains("Invalid task state"));
}
