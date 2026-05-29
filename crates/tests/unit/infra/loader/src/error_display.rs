use std::io;
use std::path::PathBuf;

use systemprompt_loader::{ConfigLoadError, ConfigWriteError, ExtensionLoadError};

#[test]
fn config_load_error_io_display() {
    let err = ConfigLoadError::Io {
        path: PathBuf::from("/some/path"),
        source: io::Error::new(io::ErrorKind::NotFound, "not found"),
    };
    let msg = err.to_string();
    assert!(msg.contains("io error"));
    assert!(msg.contains("/some/path"));
}

#[test]
fn config_load_error_yaml_display() {
    let yaml_err = serde_yaml::from_str::<serde_yaml::Value>("invalid: : :").unwrap_err();
    let err = ConfigLoadError::Yaml {
        path: PathBuf::from("/cfg/services.yaml"),
        source: yaml_err,
    };
    let msg = err.to_string();
    assert!(msg.contains("yaml parse failure"));
    assert!(msg.contains("/cfg/services.yaml"));
}

#[test]
fn config_load_error_include_not_found_display() {
    let err = ConfigLoadError::IncludeNotFound {
        include: PathBuf::from("missing/file.yaml"),
        referrer: PathBuf::from("root.yaml"),
    };
    let msg = err.to_string();
    assert!(msg.contains("include file not found"));
    assert!(msg.contains("missing/file.yaml"));
    assert!(msg.contains("root.yaml"));
}

#[test]
fn config_load_error_include_cycle_display() {
    let err = ConfigLoadError::IncludeCycle {
        chain: "a.yaml -> b.yaml -> a.yaml".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("include cycle detected"));
    assert!(msg.contains("a.yaml -> b.yaml"));
}

#[test]
fn config_load_error_duplicate_agent_display() {
    let err = ConfigLoadError::DuplicateAgent("my_agent".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("duplicate agent"));
    assert!(msg.contains("my_agent"));
}

#[test]
fn config_load_error_duplicate_mcp_server_display() {
    let err = ConfigLoadError::DuplicateMcpServer("my_mcp".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("duplicate MCP server"));
    assert!(msg.contains("my_mcp"));
}

#[test]
fn config_load_error_duplicate_plugin_display() {
    let err = ConfigLoadError::DuplicatePlugin("my_plugin".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("duplicate plugin"));
    assert!(msg.contains("my_plugin"));
}

#[test]
fn config_load_error_duplicate_marketplace_display() {
    let err = ConfigLoadError::DuplicateMarketplace("my_market".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("duplicate marketplace"));
    assert!(msg.contains("my_market"));
}

#[test]
fn config_load_error_duplicate_skill_display() {
    let err = ConfigLoadError::DuplicateSkill("my_skill".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("duplicate skill"));
    assert!(msg.contains("my_skill"));
}

#[test]
fn config_load_error_duplicate_external_agent_display() {
    let err = ConfigLoadError::DuplicateExternalAgent("ext_agent".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("duplicate external agent"));
    assert!(msg.contains("ext_agent"));
}

#[test]
fn config_load_error_validation_display() {
    let err = ConfigLoadError::Validation("port out of range".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("port out of range"));
}

#[test]
fn config_load_error_include_must_not_set_settings_display() {
    let err = ConfigLoadError::IncludeMustNotSetGlobalSettings {
        path: PathBuf::from("includes/extra.yaml"),
    };
    let msg = err.to_string();
    assert!(msg.contains("settings"));
    assert!(msg.contains("includes/extra.yaml"));
}

#[test]
fn config_write_error_io_display() {
    let err = ConfigWriteError::Io {
        path: PathBuf::from("/tmp/agent.yaml"),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("io error"));
    assert!(msg.contains("/tmp/agent.yaml"));
}

#[test]
fn config_write_error_agent_file_exists_display() {
    let err = ConfigWriteError::AgentFileExists(PathBuf::from("/agents/foo.yaml"));
    let msg = err.to_string();
    assert!(msg.contains("already exists"));
    assert!(msg.contains("/agents/foo.yaml"));
}

#[test]
fn config_write_error_agent_not_found_display() {
    let err = ConfigWriteError::AgentNotFound("my_agent".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("not found"));
    assert!(msg.contains("my_agent"));
}

#[test]
fn extension_load_error_binary_not_found_display() {
    let err = ExtensionLoadError::BinaryNotFound {
        name: "my-mcp".to_owned(),
        path: PathBuf::from("/target/release/my-mcp"),
    };
    let msg = err.to_string();
    assert!(msg.contains("my-mcp"));
    assert!(msg.contains("not found"));
}

#[test]
fn extension_load_error_manifest_missing_display() {
    let err = ExtensionLoadError::ManifestMissing("my-ext".to_owned());
    let msg = err.to_string();
    assert!(msg.contains("manifest.yaml"));
    assert!(msg.contains("my-ext"));
}

#[test]
fn error_types_implement_debug() {
    let io_err = io::Error::new(io::ErrorKind::Other, "test");
    let cl = ConfigLoadError::Io { path: PathBuf::from("/a"), source: io_err };
    let _ = format!("{cl:?}");

    let cw = ConfigWriteError::AgentNotFound("x".to_owned());
    let _ = format!("{cw:?}");

    let el = ExtensionLoadError::ManifestMissing("y".to_owned());
    let _ = format!("{el:?}");
}
