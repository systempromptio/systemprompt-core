//! Unit tests for ConfigWriter
//!
//! Tests cover:
//! - Agent file creation
//! - Agent file updating
//! - Agent file deletion
//! - Agent file finding
//! - Include management (add/remove)

use std::collections::HashMap;
use systemprompt_identifiers::AgentName;
use systemprompt_loader::{CheckError, ConfigWriteError, ConfigWriter, StagedAgentFile};
use systemprompt_manifest::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};
use tempfile::TempDir;

fn accept(_staged: &StagedAgentFile<'_>) -> Result<(), CheckError> {
    Ok(())
}

fn refuse(_staged: &StagedAgentFile<'_>) -> Result<(), CheckError> {
    Err("the configuration would be invalid".into())
}

fn tree_bytes(root: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push((path.clone(), std::fs::read(&path).expect("read file")));
            }
        }
    }
    files.sort();
    files
}

fn create_test_agent(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_string(),
        port: 4000,
        endpoint: format!("http://localhost:4000/{}", name),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: Vec::new(),
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
            security_requirements: None,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
        secrets: Vec::new(),
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
      defaultInputModes:
        - text/plain
      defaultOutputModes:
        - text/plain
    metadata: {{}}
"#,
        name, name, name, name, name
    )
}

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
    let result = ConfigWriter::create_agent(&agent, temp_dir.path(), accept);

    result.as_ref().expect("result should succeed");
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
    let result = ConfigWriter::create_agent(&agent, temp_dir.path(), accept);

    result.expect("result should succeed");
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
    let result = ConfigWriter::create_agent(&agent, temp_dir.path(), accept);

    result.as_ref().expect_err("result should fail");
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_find_agent_file_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let agent_file = agents_dir.join("my_agent.yaml");
    std::fs::write(&agent_file, create_agent_yaml_content("my_agent"))
        .expect("Failed to write agent file");

    let result = ConfigWriter::find_agent_file(&AgentName::new("my_agent"), temp_dir.path());
    result.as_ref().expect("result should succeed");
    result
        .expect("Should find agent")
        .expect("result.expect(\"Should find agent\") should be present");
}

#[test]
fn test_find_agent_file_not_exists() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let result = ConfigWriter::find_agent_file(&AgentName::new("nonexistent"), temp_dir.path());
    result.as_ref().expect("result should succeed");
    assert!(result.expect("Should return None").is_none());
}

#[test]
fn test_find_agent_file_no_agents_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let result = ConfigWriter::find_agent_file(&AgentName::new("any_agent"), temp_dir.path());
    result.as_ref().expect("result should succeed");
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
      defaultInputModes:
        - text/plain
      defaultOutputModes:
        - text/plain
    metadata: {}
"#;
    let agent_file = agents_dir.join("other-file.yaml");
    std::fs::write(&agent_file, agent_content).expect("Failed to write agent file");

    let result = ConfigWriter::find_agent_file(&AgentName::new("hidden_agent"), temp_dir.path());
    result.as_ref().expect("result should succeed");
    let found = result.expect("Should find agent");
    found.as_ref().expect("found should be present");
    assert!(
        found
            .expect("Agent found")
            .to_string_lossy()
            .contains("other-file.yaml")
    );
}

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

    let result = ConfigWriter::update_agent(
        &AgentName::new("update_agent"),
        &updated_agent,
        temp_dir.path(),
        accept,
    );
    result.expect("result should succeed");

    let content = std::fs::read_to_string(&agent_file).expect("Failed to read updated file");
    assert!(content.contains("Updated description"));
}

#[test]
fn test_update_agent_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let agent = create_test_agent("nonexistent");
    let result = ConfigWriter::update_agent(
        &AgentName::new("nonexistent"),
        &agent,
        temp_dir.path(),
        accept,
    );

    result.as_ref().expect_err("result should fail");
    assert!(result.unwrap_err().to_string().contains("not found"));
}

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

    let result = ConfigWriter::delete_agent(&AgentName::new("delete_me"), temp_dir.path());
    result.expect("result should succeed");
    assert!(!agent_file.exists());
}

#[test]
fn test_delete_agent_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let result = ConfigWriter::delete_agent(&AgentName::new("nonexistent"), temp_dir.path());
    result.as_ref().expect_err("result should fail");
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_created_agent_file_has_header() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = "includes: []\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("Failed to write config");

    let agent = create_test_agent("header_test");
    let result = ConfigWriter::create_agent(&agent, temp_dir.path(), accept);
    result.as_ref().expect("result should succeed");

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
    let result = ConfigWriter::create_agent(&agent, temp_dir.path(), accept);
    result.as_ref().expect("result should succeed");

    let agent_file = result.expect("Should create agent file");
    let content = std::fs::read_to_string(&agent_file).expect("Failed to read agent file");

    #[derive(serde::Deserialize)]
    struct AgentFile {
        agents: HashMap<String, serde_yaml::Value>,
    }

    let parsed: Result<AgentFile, _> = serde_yaml::from_str(&content);
    assert!(parsed.is_ok(), "Created file should be valid YAML");
    assert!(
        parsed
            .expect("Should parse")
            .agents
            .contains_key("yaml_test"),
        "Should contain the agent"
    );
}

#[test]
fn a_refused_update_leaves_the_tree_byte_identical() {
    let temp_dir = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(temp_dir.path().join("config")).expect("config dir");
    std::fs::write(
        temp_dir.path().join("config/config.yaml"),
        "includes:\n  - ../agents/refused.yaml\n",
    )
    .expect("config");
    std::fs::create_dir_all(temp_dir.path().join("agents")).expect("agents dir");
    std::fs::write(
        temp_dir.path().join("agents/refused.yaml"),
        create_agent_yaml_content("refused"),
    )
    .expect("agent file");
    let before = tree_bytes(temp_dir.path());

    let mut agent = create_test_agent("refused");
    agent.port = 4999;
    let error =
        ConfigWriter::update_agent(&AgentName::new("refused"), &agent, temp_dir.path(), refuse)
            .expect_err("a refused check refuses the update");
    assert!(matches!(error, ConfigWriteError::Rejected(_)), "{error}");
    assert_eq!(
        tree_bytes(temp_dir.path()),
        before,
        "nothing was left written"
    );
}

#[test]
fn a_refused_create_leaves_no_file_behind() {
    let temp_dir = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(temp_dir.path().join("config")).expect("config dir");
    std::fs::write(temp_dir.path().join("config/config.yaml"), "includes: []\n").expect("config");
    std::fs::create_dir_all(temp_dir.path().join("agents")).expect("agents dir");
    let before = tree_bytes(temp_dir.path());

    let error = ConfigWriter::create_agent(&create_test_agent("never"), temp_dir.path(), refuse)
        .expect_err("a refused check refuses the create");
    assert!(matches!(error, ConfigWriteError::Rejected(_)), "{error}");
    assert_eq!(
        tree_bytes(temp_dir.path()),
        before,
        "nothing was left written"
    );
}

#[test]
fn the_check_sees_the_staged_bytes_before_the_live_file_changes() {
    let temp_dir = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(temp_dir.path().join("agents")).expect("agents dir");
    let path = temp_dir.path().join("agents/seen.yaml");
    let probe = path.clone();
    ConfigWriter::create_agent(&create_test_agent("seen"), temp_dir.path(), move |staged| {
        assert!(!probe.exists(), "the live file is untouched while the check runs");
        assert_eq!(staged.live_path(), probe.as_path());
        let staged_bytes = std::fs::read(staged.staged_path()).expect("staged file is readable");
        assert_eq!(staged_bytes.as_slice(), staged.contents());
        Ok(())
    })
    .expect("an accepted create is kept");
    assert!(path.exists());
}

#[test]
fn a_refusing_check_never_exposes_unvalidated_bytes_and_leaves_no_staged_file() {
    let temp_dir = TempDir::new().expect("temp dir");
    let agents_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");
    let live = agents_dir.join("guarded.yaml");
    std::fs::write(&live, create_agent_yaml_content("guarded")).expect("agent file");
    let before = std::fs::read(&live).expect("live bytes");

    let mut agent = create_test_agent("guarded");
    agent.port = 4998;
    let live_probe = live.clone();
    let expected = before.clone();
    let mut staged_seen = None;
    let error = ConfigWriter::update_agent(
        &AgentName::new("guarded"),
        &agent,
        temp_dir.path(),
        |staged| {
            assert_eq!(
                std::fs::read(&live_probe).expect("live bytes during check"),
                expected,
                "the live file never shows unvalidated content"
            );
            let name = staged
                .staged_path()
                .file_name()
                .expect("staged name")
                .to_string_lossy()
                .into_owned();
            assert_ne!(name, ".guarded.yaml.staged", "the staged name is not predictable");
            assert_eq!(staged.staged_path().parent(), live_probe.parent());
            staged_seen = Some(staged.staged_path().to_path_buf());
            Err("refused".into())
        },
    )
    .expect_err("a refused check refuses the update");

    assert!(matches!(error, ConfigWriteError::Rejected(_)), "{error}");
    assert_eq!(std::fs::read(&live).expect("live bytes after"), before);
    let staged = staged_seen.expect("the check ran");
    assert!(!staged.exists(), "no staged file is left behind");
    let names: Vec<_> = std::fs::read_dir(&agents_dir)
        .expect("agents dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("guarded.yaml")]);
}
