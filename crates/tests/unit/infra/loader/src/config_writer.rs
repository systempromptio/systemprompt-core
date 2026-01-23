//! Unit tests for ConfigWriter
//!
//! Tests cover:
//! - Agent file creation
//! - Agent file updating
//! - Agent file deletion
//! - Agent file finding
//! - Include management (add/remove)

use std::collections::HashMap;
use std::path::Path;
use systemprompt_loader::ConfigWriter;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};
use tempfile::TempDir;

fn create_test_agent(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_string(),
        port: 4000,
        endpoint: format!("http://localhost:4000/{}", name),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        card: AgentCardConfig {
            protocol_version: "0.2.3".to_string(),
            name: Some(name.to_string()),
            display_name: format!("{} Agent", name),
            description: format!("Test agent {}", name),
            version: "1.0.0".to_string(),
            preferred_transport: "JSONRPC".to_string(),
            icon_url: None,
            documentation_url: None,
            provider: None,
            capabilities: CapabilitiesConfig::default(),
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            security_schemes: None,
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
    }
}

fn create_agent_yaml_content(name: &str) -> String {
    format!(
        r#"agents:
  {}:
    name: {}
    port: 4000
    endpoint: http://localhost:4000/{}
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: "{} Agent"
      description: "Test agent {}"
      version: "1.0.0"
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes:
        - text/plain
      defaultOutputModes:
        - text/plain
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {{}}
"#,
        name, name, name, name, name
    )
}

// ============================================================================
// Create Agent Tests
// ============================================================================

#[test]
fn test_create_agent_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = r#"includes: []
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let agent = create_test_agent("test_agent");
    let result = ConfigWriter::create_agent(&agent, temp_dir.path());

    assert!(result.is_ok());
    let agent_file = result.expect("Should create agent file");
    assert!(agent_file.exists());
    assert!(agent_file.to_string_lossy().contains("test_agent.yaml"));
}

#[test]
fn test_create_agent_creates_agents_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = "includes: []\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let agents_dir = temp_dir.path().join("agents");
    assert!(!agents_dir.exists());

    let agent = create_test_agent("new_agent");
    let result = ConfigWriter::create_agent(&agent, temp_dir.path());

    assert!(result.is_ok());
    assert!(agents_dir.exists());
}

#[test]
fn test_create_agent_already_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let config_content = "includes: []\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let existing_agent_file = agents_dir.join("existing.yaml");
    std::fs::write(&existing_agent_file, create_agent_yaml_content("existing"))
        .expect("Failed to write existing agent");

    let agent = create_test_agent("existing");
    let result = ConfigWriter::create_agent(&agent, temp_dir.path());

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

// ============================================================================
// Find Agent File Tests
// ============================================================================

#[test]
fn test_find_agent_file_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let agent_file = agents_dir.join("my_agent.yaml");
    std::fs::write(&agent_file, create_agent_yaml_content("my_agent"))
        .expect("Failed to write agent file");

    let result = ConfigWriter::find_agent_file("my_agent", temp_dir.path());
    assert!(result.is_ok());
    assert!(result.expect("Should find agent").is_some());
}

#[test]
fn test_find_agent_file_not_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let result = ConfigWriter::find_agent_file("nonexistent", temp_dir.path());
    assert!(result.is_ok());
    assert!(result.expect("Should return None").is_none());
}

#[test]
fn test_find_agent_file_no_agents_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let result = ConfigWriter::find_agent_file("any_agent", temp_dir.path());
    assert!(result.is_ok());
    assert!(result.expect("Should return None").is_none());
}

#[test]
fn test_find_agent_file_in_different_filename() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let agent_content = r#"agents:
  hidden_agent:
    name: hidden_agent
    port: 4000
    endpoint: http://localhost:4000/hidden_agent
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: "Hidden Agent"
      description: "Agent in different file"
      version: "1.0.0"
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes:
        - text/plain
      defaultOutputModes:
        - text/plain
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {}
"#;
    let agent_file = agents_dir.join("other-file.yaml");
    std::fs::write(&agent_file, agent_content).expect("Failed to write agent file");

    let result = ConfigWriter::find_agent_file("hidden_agent", temp_dir.path());
    assert!(result.is_ok());
    let found = result.expect("Should find agent");
    assert!(found.is_some());
    assert!(
        found
            .expect("Agent found")
            .to_string_lossy()
            .contains("other-file.yaml")
    );
}

// ============================================================================
// Update Agent Tests
// ============================================================================

#[test]
fn test_update_agent_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let agent_file = agents_dir.join("update_agent.yaml");
    std::fs::write(&agent_file, create_agent_yaml_content("update_agent"))
        .expect("Failed to write agent file");

    let mut updated_agent = create_test_agent("update_agent");
    updated_agent.card.description = "Updated description".to_string();

    let result = ConfigWriter::update_agent("update_agent", &updated_agent, temp_dir.path());
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&agent_file).expect("Failed to read updated file");
    assert!(content.contains("Updated description"));
}

#[test]
fn test_update_agent_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let agent = create_test_agent("nonexistent");
    let result = ConfigWriter::update_agent("nonexistent", &agent, temp_dir.path());

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ============================================================================
// Delete Agent Tests
// ============================================================================

#[test]
fn test_delete_agent_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let config_content = "includes:\n  - ../agents/delete_me.yaml\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let agent_file = agents_dir.join("delete_me.yaml");
    std::fs::write(&agent_file, create_agent_yaml_content("delete_me"))
        .expect("Failed to write agent file");

    assert!(agent_file.exists());

    let result = ConfigWriter::delete_agent("delete_me", temp_dir.path());
    assert!(result.is_ok());
    assert!(!agent_file.exists());
}

#[test]
fn test_delete_agent_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let result = ConfigWriter::delete_agent("nonexistent", temp_dir.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ============================================================================
// Add Include Tests
// ============================================================================

#[test]
fn test_add_include_to_existing_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.yaml");

    let config_content = "includes:\n  - existing.yaml\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(&config_path, config_content).expect("Failed to write config");

    let result = ConfigWriter::add_include("new-include.yaml", &config_path);
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
    assert!(content.contains("new-include.yaml"));
    assert!(content.contains("existing.yaml"));
}

#[test]
fn test_add_include_creates_includes_section() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.yaml");

    let config_content = "agents: {}\nmcp_servers: {}\n";
    std::fs::write(&config_path, config_content).expect("Failed to write config");

    let result = ConfigWriter::add_include("new-include.yaml", &config_path);
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
    assert!(content.contains("includes:"));
    assert!(content.contains("new-include.yaml"));
}

#[test]
fn test_add_include_already_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.yaml");

    let config_content = "includes:\n  - already-there.yaml\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(&config_path, config_content).expect("Failed to write config");

    let result = ConfigWriter::add_include("already-there.yaml", &config_path);
    assert!(result.is_ok());

    let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
    let count = content.matches("already-there.yaml").count();
    assert_eq!(count, 1, "Should not duplicate the include");
}

#[test]
fn test_add_include_nonexistent_config() {
    let path = Path::new("/nonexistent/config.yaml");
    let result = ConfigWriter::add_include("include.yaml", path);
    assert!(result.is_err());
}

// ============================================================================
// Agent File Content Tests
// ============================================================================

#[test]
fn test_created_agent_file_has_header() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = "includes: []\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let agent = create_test_agent("header_test");
    let result = ConfigWriter::create_agent(&agent, temp_dir.path());
    assert!(result.is_ok());

    let agent_file = result.expect("Should create agent file");
    let content = std::fs::read_to_string(&agent_file).expect("Failed to read agent file");

    assert!(
        content.starts_with("#"),
        "File should start with a comment header"
    );
    assert!(
        content.contains("header_test Agent"),
        "Header should contain display name"
    );
}

#[test]
fn test_created_agent_file_is_valid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = "includes: []\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let agent = create_test_agent("yaml_test");
    let result = ConfigWriter::create_agent(&agent, temp_dir.path());
    assert!(result.is_ok());

    let agent_file = result.expect("Should create agent file");
    let content = std::fs::read_to_string(&agent_file).expect("Failed to read agent file");

    #[derive(serde::Deserialize)]
    struct AgentFile {
        agents: HashMap<String, serde_yaml::Value>,
    }

    let parsed: Result<AgentFile, _> = serde_yaml::from_str(&content);
    assert!(parsed.is_ok(), "Created file should be valid YAML");
    assert!(
        parsed.expect("Should parse").agents.contains_key("yaml_test"),
        "Should contain the agent"
    );
}
