//! Unit tests for runtime models
//!
//! Tests cover:
//! - AgentRuntimeInfo serialization and deserialization
//! - Conversion from AgentConfig

use systemprompt_agent::models::runtime::AgentRuntimeInfo;
use systemprompt_models::ai::ToolModelOverrides;

// ============================================================================
// AgentRuntimeInfo Tests
// ============================================================================

#[test]
fn test_agent_runtime_info_serialize() {
    let info = AgentRuntimeInfo {
        name: "test-agent".to_string(),
        port: 8080,
        is_enabled: true,
        is_primary: false,
        system_prompt: Some("You are a helpful assistant".to_string()),
        mcp_servers: vec!["server1".to_string(), "server2".to_string()],
        provider: Some("anthropic".to_string()),
        model: Some("claude-3-opus".to_string()),
        skills: vec!["skill1".to_string()],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("test-agent"));
    assert!(json.contains("8080"));
    assert!(json.contains("isEnabled"));
    assert!(json.contains("isPrimary"));
}

#[test]
fn test_agent_runtime_info_deserialize() {
    let json = r#"{
        "name": "my-agent",
        "port": 9000,
        "isEnabled": true,
        "isPrimary": true,
        "systemPrompt": "System prompt here",
        "mcpServers": ["mcp1"],
        "provider": "openai",
        "model": "gpt-4",
        "skills": [],
        "toolModelOverrides": {}
    }"#;

    let info: AgentRuntimeInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.name, "my-agent");
    assert_eq!(info.port, 9000);
    assert!(info.is_enabled);
    assert!(info.is_primary);
    assert_eq!(info.system_prompt, Some("System prompt here".to_string()));
    assert_eq!(info.mcp_servers, vec!["mcp1".to_string()]);
    assert_eq!(info.provider, Some("openai".to_string()));
    assert_eq!(info.model, Some("gpt-4".to_string()));
}

#[test]
fn test_agent_runtime_info_optional_fields() {
    let json = r#"{
        "name": "minimal-agent",
        "port": 3000,
        "isEnabled": false,
        "isPrimary": false,
        "mcpServers": [],
        "skills": [],
        "toolModelOverrides": {}
    }"#;

    let info: AgentRuntimeInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.name, "minimal-agent");
    assert_eq!(info.port, 3000);
    assert!(!info.is_enabled);
    assert!(!info.is_primary);
    assert!(info.system_prompt.is_none());
    assert!(info.provider.is_none());
    assert!(info.model.is_none());
    assert!(info.mcp_servers.is_empty());
}

#[test]
fn test_agent_runtime_info_equality() {
    let info1 = AgentRuntimeInfo {
        name: "agent-eq".to_string(),
        port: 5000,
        is_enabled: true,
        is_primary: true,
        system_prompt: None,
        mcp_servers: vec![],
        provider: None,
        model: None,
        skills: vec![],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    let info2 = AgentRuntimeInfo {
        name: "agent-eq".to_string(),
        port: 5000,
        is_enabled: true,
        is_primary: true,
        system_prompt: None,
        mcp_servers: vec![],
        provider: None,
        model: None,
        skills: vec![],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    assert_eq!(info1, info2);
}

#[test]
fn test_agent_runtime_info_inequality() {
    let info1 = AgentRuntimeInfo {
        name: "agent-1".to_string(),
        port: 5000,
        is_enabled: true,
        is_primary: true,
        system_prompt: None,
        mcp_servers: vec![],
        provider: None,
        model: None,
        skills: vec![],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    let info2 = AgentRuntimeInfo {
        name: "agent-2".to_string(),
        port: 5000,
        is_enabled: true,
        is_primary: true,
        system_prompt: None,
        mcp_servers: vec![],
        provider: None,
        model: None,
        skills: vec![],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    assert_ne!(info1, info2);
}

#[test]
fn test_agent_runtime_info_debug() {
    let info = AgentRuntimeInfo {
        name: "debug-agent".to_string(),
        port: 8888,
        is_enabled: true,
        is_primary: false,
        system_prompt: None,
        mcp_servers: vec![],
        provider: None,
        model: None,
        skills: vec![],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    let debug_str = format!("{:?}", info);
    assert!(debug_str.contains("AgentRuntimeInfo"));
    assert!(debug_str.contains("debug-agent"));
    assert!(debug_str.contains("8888"));
}

#[test]
fn test_agent_runtime_info_clone() {
    let info = AgentRuntimeInfo {
        name: "clone-agent".to_string(),
        port: 7777,
        is_enabled: true,
        is_primary: true,
        system_prompt: Some("Cloned prompt".to_string()),
        mcp_servers: vec!["server".to_string()],
        provider: Some("provider".to_string()),
        model: Some("model".to_string()),
        skills: vec!["skill".to_string()],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    let cloned = info.clone();
    assert_eq!(cloned.name, info.name);
    assert_eq!(cloned.port, info.port);
    assert_eq!(cloned.system_prompt, info.system_prompt);
    assert_eq!(cloned.mcp_servers, info.mcp_servers);
}

#[test]
fn test_agent_runtime_info_with_skills() {
    let info = AgentRuntimeInfo {
        name: "skilled-agent".to_string(),
        port: 6666,
        is_enabled: true,
        is_primary: false,
        system_prompt: None,
        mcp_servers: vec![],
        provider: None,
        model: None,
        skills: vec![
            "code-review".to_string(),
            "documentation".to_string(),
            "testing".to_string(),
        ],
        tool_model_overrides: ToolModelOverrides::default(),
    };

    assert_eq!(info.skills.len(), 3);
    assert!(info.skills.contains(&"code-review".to_string()));
    assert!(info.skills.contains(&"documentation".to_string()));
    assert!(info.skills.contains(&"testing".to_string()));
}
