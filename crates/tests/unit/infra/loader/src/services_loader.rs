//! Unit tests for ConfigLoader

use std::path::PathBuf;
use systemprompt_loader::ConfigLoader;
use tempfile::TempDir;

fn create_minimal_config() -> String {
    r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#
    .to_string()
}

#[test]
fn test_load_from_path_minimal_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let config = ConfigLoader::load_from_path(&config_path).expect("Should load config");
    assert!(config.agents.is_empty());
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_load_from_path_nonexistent() {
    let path = PathBuf::from("/nonexistent/services.yaml");
    let result = ConfigLoader::load_from_path(&path);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to read"));
}

#[test]
fn test_load_from_path_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, "invalid: yaml: : :").expect("Failed to write config");

    ConfigLoader::load_from_path(&config_path).unwrap_err();
}

#[test]
fn test_load_from_content_minimal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    ConfigLoader::load_from_content(&create_minimal_config(), &config_path)
        .expect("result should succeed");
}

#[test]
fn test_load_from_content_with_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let include_content = r#"
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("agents.yaml"), include_content)
        .expect("Failed to write include");

    let main_content = r#"
includes:
  - agents.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    ConfigLoader::load_from_content(main_content, &config_path).expect("result should succeed");
}

#[test]
fn test_validate_file_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    ConfigLoader::validate_file(&config_path).expect("result should succeed");
}

#[test]
fn test_loader_new_base_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("config");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let config_path = subdir.join("services.yaml");

    let loader = ConfigLoader::new(config_path);
    assert_eq!(loader.base_path(), subdir);
}

#[test]
fn test_loader_get_includes_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let loader = ConfigLoader::new(config_path);
    let includes = loader.get_includes().expect("Should get includes");
    assert!(includes.is_empty());
}

#[test]
fn test_loader_get_includes_with_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let content = r#"
includes:
  - agents.yaml
  - mcp-servers.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    std::fs::write(&config_path, content).expect("Failed to write config");

    let loader = ConfigLoader::new(config_path);
    let includes = loader.get_includes().expect("Should get includes");
    assert_eq!(includes.len(), 2);
    assert!(includes.contains(&"agents.yaml".to_string()));
    assert!(includes.contains(&"mcp-servers.yaml".to_string()));
}

#[test]
fn test_loader_list_all_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(
        temp_dir.path().join("existing.yaml"),
        "agents: {}\nmcp_servers: {}",
    )
    .expect("Failed to write include");

    let content = r#"
includes:
  - existing.yaml
  - missing.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    std::fs::write(&config_path, content).expect("Failed to write config");

    let loader = ConfigLoader::new(config_path);
    let includes = loader.list_all_includes().expect("Should list includes");
    assert_eq!(includes.len(), 2);

    let existing = includes
        .iter()
        .find(|(name, _)| name == "existing.yaml")
        .expect("existing should be present");
    assert!(existing.1);

    let missing = includes
        .iter()
        .find(|(name, _)| name == "missing.yaml")
        .expect("missing should be present");
    assert!(!missing.1);
}

#[test]
fn test_merge_multiple_includes_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let empty_partial = "agents: {}\nmcp_servers: {}\n";
    std::fs::write(temp_dir.path().join("agents1.yaml"), empty_partial)
        .expect("Failed to write agents1");
    std::fs::write(temp_dir.path().join("agents2.yaml"), empty_partial)
        .expect("Failed to write agents2");

    let main_content = r#"
includes:
  - agents1.yaml
  - agents2.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    std::fs::write(&config_path, main_content).expect("Failed to write config");

    let config = ConfigLoader::load_from_path(&config_path).expect("Should load merged config");
    assert!(config.agents.is_empty());
    assert!(config.mcp_servers.is_empty());
}
