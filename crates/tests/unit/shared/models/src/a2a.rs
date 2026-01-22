//! Unit tests for A2A protocol models
//!
//! Tests cover:
//! - AgentCard creation, serialization, and builder pattern
//! - AgentCapabilities default values and normalization
//! - AgentExtension factory methods
//! - AgentSkill from MCP server
//! - Task and TaskStatus default values
//! - TaskState enum variants and parsing
//! - Message and Part types serialization

use systemprompt_models::{
    AgentCapabilities, AgentCard, AgentExtension, AgentProvider, AgentSkill, Task, TaskState,
    TaskStatus,
};

// ============================================================================
// AgentCard Tests
// ============================================================================

#[test]
fn test_agent_card_builder_creates_valid_card() {
    let card = AgentCard::builder(
        "Test Agent".to_string(),
        "A test agent".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert_eq!(card.name, "Test Agent");
    assert_eq!(card.description, "A test agent");
    assert_eq!(card.url, "https://example.com");
    assert_eq!(card.version, "1.0.0");
    assert_eq!(card.protocol_version, "0.3.0");
}

#[test]
fn test_agent_card_builder_with_provider() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .with_provider("systemprompt.io".to_string(), "https://systemprompt.io".to_string())
    .build();

    assert!(card.provider.is_some());
    let provider = card.provider.unwrap();
    assert_eq!(provider.organization, "systemprompt.io");
    assert_eq!(provider.url, "https://systemprompt.io");
}

#[test]
fn test_agent_card_builder_with_streaming() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .with_streaming()
    .build();

    assert_eq!(card.capabilities.streaming, Some(true));
}

#[test]
fn test_agent_card_builder_with_push_notifications() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .with_push_notifications()
    .build();

    assert_eq!(card.capabilities.push_notifications, Some(true));
}

#[test]
fn test_agent_card_default_input_output_modes() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert!(card.default_input_modes.contains(&"text/plain".to_string()));
    assert!(card.default_output_modes.contains(&"text/plain".to_string()));
}

#[test]
fn test_agent_card_serialize_deserialize() {
    let card = AgentCard::builder(
        "Test Agent".to_string(),
        "A test agent".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    let json = serde_json::to_string(&card).unwrap();
    let deserialized: AgentCard = serde_json::from_str(&json).unwrap();

    assert_eq!(card.name, deserialized.name);
    assert_eq!(card.description, deserialized.description);
    assert_eq!(card.url, deserialized.url);
    assert_eq!(card.version, deserialized.version);
}

#[test]
fn test_agent_card_has_mcp_extension_false() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert!(!card.has_mcp_extension());
}

#[test]
fn test_agent_card_ensure_mcp_extension() {
    let mut card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert!(!card.has_mcp_extension());
    card.ensure_mcp_extension();
    assert!(card.has_mcp_extension());
}

#[test]
fn test_agent_card_ensure_mcp_extension_idempotent() {
    let mut card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    card.ensure_mcp_extension();
    let ext_count = card.capabilities.extensions.as_ref().map(|e| e.len()).unwrap_or(0);

    card.ensure_mcp_extension();
    let ext_count_after = card.capabilities.extensions.as_ref().map(|e| e.len()).unwrap_or(0);

    assert_eq!(ext_count, ext_count_after);
}

// ============================================================================
// AgentCapabilities Tests
// ============================================================================

#[test]
fn test_agent_capabilities_default() {
    let caps = AgentCapabilities::default();

    assert_eq!(caps.streaming, Some(true));
    assert_eq!(caps.push_notifications, Some(true));
    assert_eq!(caps.state_transition_history, Some(true));
    assert!(caps.extensions.is_none());
}

#[test]
fn test_agent_capabilities_normalize_none_values() {
    let caps = AgentCapabilities {
        streaming: None,
        push_notifications: None,
        state_transition_history: None,
        extensions: None,
    };

    let normalized = caps.normalize();

    assert_eq!(normalized.streaming, Some(true));
    assert_eq!(normalized.push_notifications, Some(false));
    assert_eq!(normalized.state_transition_history, Some(true));
}

#[test]
fn test_agent_capabilities_normalize_preserves_existing() {
    let caps = AgentCapabilities {
        streaming: Some(false),
        push_notifications: Some(true),
        state_transition_history: Some(false),
        extensions: None,
    };

    let normalized = caps.normalize();

    assert_eq!(normalized.streaming, Some(false));
    assert_eq!(normalized.push_notifications, Some(true));
    assert_eq!(normalized.state_transition_history, Some(false));
}

// ============================================================================
// AgentExtension Tests
// ============================================================================

#[test]
fn test_agent_extension_mcp_tools() {
    let ext = AgentExtension::mcp_tools_extension();

    assert_eq!(ext.uri, "systemprompt:mcp-tools");
    assert!(ext.description.is_some());
    assert_eq!(ext.required, Some(false));
    assert!(ext.params.is_some());
}

#[test]
fn test_agent_extension_mcp_tools_with_servers() {
    let servers = vec![serde_json::json!({"name": "test-server"})];
    let ext = AgentExtension::mcp_tools_extension_with_servers(&servers);

    assert_eq!(ext.uri, "systemprompt:mcp-tools");
    let params = ext.params.unwrap();
    assert!(params.get("servers").is_some());
}

#[test]
fn test_agent_extension_opencode_integration() {
    let ext = AgentExtension::opencode_integration_extension();

    assert_eq!(ext.uri, "systemprompt:opencode-integration");
    assert!(ext.description.is_some());
}

#[test]
fn test_agent_extension_artifact_rendering() {
    let ext = AgentExtension::artifact_rendering_extension();

    assert!(ext.uri.contains("artifact-rendering"));
    let params = ext.params.unwrap();
    assert!(params.get("supported_types").is_some());
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
    assert_eq!(params.get("systemPrompt").unwrap(), "You are a helpful assistant");
}

#[test]
fn test_agent_extension_system_instructions_opt_some() {
    let ext = AgentExtension::system_instructions_opt(Some("test prompt"));
    assert!(ext.is_some());
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

    assert_eq!(task.kind, "task");
    assert!(task.history.is_none());
    assert!(task.artifacts.is_none());
    assert!(task.metadata.is_none());
    assert!(matches!(task.status.state, TaskState::Submitted));
}

#[test]
fn test_task_serialize_deserialize() {
    let task = Task::default();

    let json = serde_json::to_string(&task).unwrap();
    let deserialized: Task = serde_json::from_str(&json).unwrap();

    assert_eq!(task.id, deserialized.id);
    assert_eq!(task.kind, deserialized.kind);
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
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid task state"));
}

#[test]
fn test_task_state_serialize() {
    let json = serde_json::to_string(&TaskState::Completed).unwrap();
    assert_eq!(json, "\"completed\"");
}

#[test]
fn test_task_state_deserialize() {
    let state: TaskState = serde_json::from_str("\"working\"").unwrap();
    assert!(matches!(state, TaskState::Working));
}

// ============================================================================
// AgentProvider Tests
// ============================================================================

#[test]
fn test_agent_provider_serialize() {
    let provider = AgentProvider {
        organization: "TestOrg".to_string(),
        url: "https://test.org".to_string(),
    };

    let json = serde_json::to_string(&provider).unwrap();
    assert!(json.contains("TestOrg"));
    assert!(json.contains("https://test.org"));
}

#[test]
fn test_agent_provider_deserialize() {
    let json = r#"{"organization":"TestOrg","url":"https://test.org"}"#;
    let provider: AgentProvider = serde_json::from_str(json).unwrap();

    assert_eq!(provider.organization, "TestOrg");
    assert_eq!(provider.url, "https://test.org");
}

#[test]
fn test_agent_provider_equality() {
    let p1 = AgentProvider {
        organization: "Org".to_string(),
        url: "https://org.com".to_string(),
    };
    let p2 = AgentProvider {
        organization: "Org".to_string(),
        url: "https://org.com".to_string(),
    };

    assert_eq!(p1, p2);
}
