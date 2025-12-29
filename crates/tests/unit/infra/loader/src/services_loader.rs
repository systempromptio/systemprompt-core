//! Unit tests for ConfigLoader and EnhancedConfigLoader
//!
//! Tests cover:
//! - Config loading from file path
//! - Config loading from content
//! - Config validation
//! - EnhancedConfigLoader functionality

use std::path::PathBuf;
use systemprompt_loader::{ConfigLoader, EnhancedConfigLoader};
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_minimal_config() -> String {
    r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
web:
  enabled: false
"#
    .to_string()
}

// ============================================================================
// ConfigLoader - Load From Path Tests
// ============================================================================

#[test]
fn test_load_from_path_minimal_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let result = ConfigLoader::load_from_path(&config_path);
    assert!(result.is_ok());

    let config = result.expect("Should load config");
    assert!(config.agents.is_empty());
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_load_from_path_nonexistent() {
    let path = PathBuf::from("/nonexistent/services.yaml");
    let result = ConfigLoader::load_from_path(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to read"));
}

#[test]
fn test_load_from_path_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, "invalid: yaml: : :").expect("Failed to write config");

    let result = ConfigLoader::load_from_path(&config_path);
    assert!(result.is_err());
}

// ============================================================================
// ConfigLoader - Load From Content Tests
// ============================================================================

#[test]
fn test_load_from_content_minimal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let result = ConfigLoader::load_from_content(&create_minimal_config(), &config_path);
    assert!(result.is_ok());
}

#[test]
fn test_load_from_content_with_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    // Create empty include file (agents and mcp_servers are empty)
    let include_content = r#"
agents: {}
mcp_servers: {}
"#;
    let include_path = temp_dir.path().join("agents.yaml");
    std::fs::write(&include_path, include_content).expect("Failed to write include");

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
web:
  enabled: false
"#;

    let result = ConfigLoader::load_from_content(main_content, &config_path);
    assert!(result.is_ok());
}

// ============================================================================
// ConfigLoader - Validation Tests
// ============================================================================

#[test]
fn test_validate_file_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let result = ConfigLoader::validate_file(&config_path);
    assert!(result.is_ok());
}

// ============================================================================
// EnhancedConfigLoader - Constructor Tests
// ============================================================================

#[test]
fn test_enhanced_loader_new() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let loader = EnhancedConfigLoader::new(config_path.clone());
    assert_eq!(loader.base_path(), temp_dir.path());
}

#[test]
fn test_enhanced_loader_base_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("config");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let config_path = subdir.join("services.yaml");

    let loader = EnhancedConfigLoader::new(config_path);
    assert_eq!(loader.base_path(), subdir);
}

// ============================================================================
// EnhancedConfigLoader - Load Tests
// ============================================================================

#[test]
fn test_enhanced_loader_load() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.load();
    assert!(result.is_ok());
}

#[test]
fn test_enhanced_loader_load_from_content() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.load_from_content(&create_minimal_config());
    assert!(result.is_ok());
}

#[test]
fn test_enhanced_loader_with_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    // Create empty include file
    let include_content = r#"
agents: {}
mcp_servers: {}
"#;
    let include_path = temp_dir.path().join("enhanced-agents.yaml");
    std::fs::write(&include_path, include_content).expect("Failed to write include");

    let main_content = r#"
includes:
  - enhanced-agents.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
web:
  enabled: false
"#;

    std::fs::write(&config_path, main_content).expect("Failed to write config");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.load();
    assert!(result.is_ok());
}

// ============================================================================
// EnhancedConfigLoader - Get Includes Tests
// ============================================================================

#[test]
fn test_enhanced_loader_get_includes_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.get_includes();
    assert!(result.is_ok());
    assert!(result.expect("Should get includes").is_empty());
}

#[test]
fn test_enhanced_loader_get_includes_with_files() {
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
web:
  enabled: false
"#;

    std::fs::write(&config_path, content).expect("Failed to write config");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.get_includes();
    assert!(result.is_ok());

    let includes = result.expect("Should get includes");
    assert_eq!(includes.len(), 2);
    assert!(includes.contains(&"agents.yaml".to_string()));
    assert!(includes.contains(&"mcp-servers.yaml".to_string()));
}

#[test]
fn test_enhanced_loader_list_all_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    // Create one include file
    let include_path = temp_dir.path().join("existing.yaml");
    std::fs::write(&include_path, "agents: {}\nmcp_servers: {}").expect("Failed to write include");

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
web:
  enabled: false
"#;

    std::fs::write(&config_path, content).expect("Failed to write config");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.list_all_includes();
    assert!(result.is_ok());

    let includes = result.expect("Should list includes");
    assert_eq!(includes.len(), 2);

    // Find the existing and missing includes
    let existing = includes.iter().find(|(name, _)| name == "existing.yaml");
    let missing = includes.iter().find(|(name, _)| name == "missing.yaml");

    assert!(existing.is_some());
    assert!(existing.expect("Has existing").1); // exists = true

    assert!(missing.is_some());
    assert!(!missing.expect("Has missing").1); // exists = false
}

// ============================================================================
// EnhancedConfigLoader - Validate File Tests
// ============================================================================

#[test]
fn test_enhanced_loader_validate_file_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let result = EnhancedConfigLoader::validate_file(&config_path);
    assert!(result.is_ok());
}

// ============================================================================
// Include Merge Tests
// ============================================================================

#[test]
fn test_merge_multiple_includes_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    // Create include files with empty agents and mcp_servers
    let agents1_content = r#"
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("agents1.yaml"), agents1_content)
        .expect("Failed to write agents1");

    let agents2_content = r#"
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("agents2.yaml"), agents2_content)
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
web:
  enabled: false
"#;

    std::fs::write(&config_path, main_content).expect("Failed to write config");

    let loader = EnhancedConfigLoader::new(config_path);
    let result = loader.load();
    assert!(result.is_ok());

    let config = result.expect("Should load merged config");
    assert!(config.agents.is_empty());
    assert!(config.mcp_servers.is_empty());
}
