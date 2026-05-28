//! Unit tests for runtime models
//!
//! Tests cover:
//! - AgentRuntimeInfo serialization and deserialization
//! - Conversion from AgentConfig

use systemprompt_agent::models::runtime::AgentRuntimeInfo;
use systemprompt_models::ai::ToolModelOverrides;
use systemprompt_models::services::PluginComponentRef;

fn pcr<I: IntoIterator<Item = &'static str>>(items: I) -> PluginComponentRef {
    PluginComponentRef {
        include: items.into_iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn test_agent_runtime_info_serialize() {
    let info = AgentRuntimeInfo {
        name: "test-agent".to_string(),
        port: 8080,
        is_enabled: true,
        is_primary: false,
        system_prompt: Some("You are a helpful assistant".to_string()),
        mcp_servers: pcr(["server1", "server2"]),
        provider: Some("anthropic".to_string()),
        model: Some("claude-3-opus".to_string()),
        max_output_tokens: Some(4096),
        skills: pcr(["skill1"]),
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
        "mcpServers": {"include": ["mcp1"]},
        "provider": "openai",
        "model": "gpt-4",
        "maxOutputTokens": 8192,
        "skills": {},
        "toolModelOverrides": {}
    }"#;

    let info: AgentRuntimeInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.name, "my-agent");
    assert_eq!(info.port, 9000);
    assert!(info.is_enabled);
    assert!(info.is_primary);
    assert_eq!(info.system_prompt, Some("System prompt here".to_string()));
    assert_eq!(info.mcp_servers.include, vec!["mcp1".to_string()]);
    assert_eq!(info.provider, Some("openai".to_string()));
    assert_eq!(info.model, Some("gpt-4".to_string()));
    assert_eq!(info.max_output_tokens, Some(8192));
}

#[test]
fn test_agent_runtime_info_optional_fields() {
    let json = r#"{
        "name": "minimal-agent",
        "port": 3000,
        "isEnabled": false,
        "isPrimary": false,
        "mcpServers": {},
        "skills": {},
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
    assert!(info.max_output_tokens.is_none());
    assert!(info.mcp_servers.include.is_empty());
}

#[test]
fn test_agent_runtime_info_debug() {
    let info = AgentRuntimeInfo {
        name: "debug-agent".to_string(),
        port: 8888,
        is_enabled: true,
        is_primary: false,
        system_prompt: None,
        mcp_servers: PluginComponentRef::default(),
        provider: None,
        model: None,
        max_output_tokens: None,
        skills: PluginComponentRef::default(),
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
        mcp_servers: pcr(["server"]),
        provider: Some("provider".to_string()),
        model: Some("model".to_string()),
        max_output_tokens: Some(2048),
        skills: pcr(["skill"]),
        tool_model_overrides: ToolModelOverrides::default(),
    };

    let cloned = info.clone();
    assert_eq!(cloned.name, info.name);
    assert_eq!(cloned.port, info.port);
    assert_eq!(cloned.system_prompt, info.system_prompt);
    assert_eq!(cloned.mcp_servers, info.mcp_servers);
    assert_eq!(cloned.max_output_tokens, info.max_output_tokens);
}

#[test]
fn test_agent_runtime_info_with_skills() {
    let info = AgentRuntimeInfo {
        name: "skilled-agent".to_string(),
        port: 6666,
        is_enabled: true,
        is_primary: false,
        system_prompt: None,
        mcp_servers: PluginComponentRef::default(),
        provider: None,
        model: None,
        max_output_tokens: None,
        skills: pcr(["code-review", "documentation", "testing"]),
        tool_model_overrides: ToolModelOverrides::default(),
    };

    assert_eq!(info.skills.include.len(), 3);
    assert!(info.skills.include.contains(&"code-review".to_string()));
    assert!(info.skills.include.contains(&"documentation".to_string()));
    assert!(info.skills.include.contains(&"testing".to_string()));
}
