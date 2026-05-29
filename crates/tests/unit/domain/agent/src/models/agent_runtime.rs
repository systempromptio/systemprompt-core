use systemprompt_agent::models::AgentRuntimeInfo;
use systemprompt_models::ai::ToolModelOverrides;
use systemprompt_models::services::PluginComponentRef;

fn minimal_runtime_info(name: &str, port: u16) -> AgentRuntimeInfo {
    AgentRuntimeInfo {
        name: name.to_string(),
        port,
        is_enabled: true,
        is_primary: false,
        system_prompt: None,
        mcp_servers: PluginComponentRef::default(),
        provider: None,
        model: None,
        max_output_tokens: None,
        skills: PluginComponentRef::default(),
        tool_model_overrides: ToolModelOverrides::default(),
    }
}

#[test]
fn agent_runtime_info_fields_accessible() {
    let info = minimal_runtime_info("my-agent", 8080);
    assert_eq!(info.name, "my-agent");
    assert_eq!(info.port, 8080);
    assert!(info.is_enabled);
    assert!(!info.is_primary);
    assert!(info.system_prompt.is_none());
    assert!(info.provider.is_none());
    assert!(info.model.is_none());
    assert!(info.max_output_tokens.is_none());
}

#[test]
fn agent_runtime_info_serde_roundtrip() {
    let info = AgentRuntimeInfo {
        name: "serde-agent".to_string(),
        port: 9000,
        is_enabled: true,
        is_primary: true,
        system_prompt: Some("You are helpful".to_string()),
        mcp_servers: PluginComponentRef::default(),
        provider: Some("anthropic".to_string()),
        model: Some("claude-3".to_string()),
        max_output_tokens: Some(4096),
        skills: PluginComponentRef::default(),
        tool_model_overrides: ToolModelOverrides::default(),
    };
    let json = serde_json::to_string(&info).unwrap();
    let de: AgentRuntimeInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(de.name, "serde-agent");
    assert_eq!(de.port, 9000);
    assert_eq!(de.provider, Some("anthropic".to_string()));
    assert_eq!(de.model, Some("claude-3".to_string()));
    assert_eq!(de.max_output_tokens, Some(4096));
    assert!(de.is_primary);
}

#[test]
fn agent_runtime_info_without_optional_fields_serde() {
    let info = minimal_runtime_info("minimal", 8080);
    let json = serde_json::to_string(&info).unwrap();
    let de: AgentRuntimeInfo = serde_json::from_str(&json).unwrap();
    assert!(de.system_prompt.is_none());
    assert!(de.provider.is_none());
    assert!(de.model.is_none());
    assert!(de.max_output_tokens.is_none());
}

#[test]
fn agent_runtime_info_debug() {
    let info = minimal_runtime_info("debug-agent", 7000);
    let dbg = format!("{:?}", info);
    assert!(dbg.contains("AgentRuntimeInfo"));
    assert!(dbg.contains("debug-agent"));
}

#[test]
fn agent_runtime_info_clone_and_eq() {
    let info = minimal_runtime_info("clone-me", 5000);
    let cloned = info.clone();
    assert_eq!(cloned, info);
}

#[test]
fn agent_runtime_info_with_system_prompt() {
    let info = AgentRuntimeInfo {
        name: "prompted".to_string(),
        port: 8080,
        is_enabled: true,
        is_primary: false,
        system_prompt: Some("You are a coding assistant".to_string()),
        mcp_servers: PluginComponentRef::default(),
        provider: None,
        model: None,
        max_output_tokens: None,
        skills: PluginComponentRef::default(),
        tool_model_overrides: ToolModelOverrides::default(),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("You are a coding assistant"));
    let de: AgentRuntimeInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(de.system_prompt, Some("You are a coding assistant".to_string()));
}

#[test]
fn agent_runtime_info_disabled_not_primary() {
    let info = AgentRuntimeInfo {
        name: "disabled".to_string(),
        port: 8080,
        is_enabled: false,
        is_primary: false,
        system_prompt: None,
        mcp_servers: PluginComponentRef::default(),
        provider: None,
        model: None,
        max_output_tokens: None,
        skills: PluginComponentRef::default(),
        tool_model_overrides: ToolModelOverrides::default(),
    };
    assert!(!info.is_enabled);
    assert!(!info.is_primary);
}

#[test]
fn agent_runtime_info_ne_different_ports() {
    let a = minimal_runtime_info("agent", 8080);
    let mut b = minimal_runtime_info("agent", 9000);
    b.port = 9000;
    assert_ne!(a, b);
}
