//! Tests for single-agent deletion: the definition and its include go
//! together, a failure keeps both, and a running agent is refused.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use systemprompt_agent::services::config_authoring::AgentConfigAuthoringService;
use systemprompt_agent_entry::cli::agents::delete::{AgentDeleteError, delete_single_agent};
use systemprompt_identifiers::AgentName;
use systemprompt_manifest::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};

fn agent(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_owned(),
        port: 9001,
        endpoint: "/a2a".to_owned(),
        enabled: false,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: vec![],
        card: AgentCardConfig {
            protocol_version: "1.0".to_owned(),
            name: None,
            display_name: "Doomed".to_owned(),
            description: "Doomed agent".to_owned(),
            version: "1.0.0".to_owned(),
            preferred_transport: "JSONRPC".to_owned(),
            icon_url: None,
            documentation_url: None,
            provider: None,
            capabilities: CapabilitiesConfig::default(),
            default_input_modes: vec!["text/plain".to_owned()],
            default_output_modes: vec!["text/plain".to_owned()],
            security_schemes: None,
            security_requirements: None,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
        secrets: Vec::new(),
    }
}

fn write_agent(services: &Path, name: &str) {
    let agents_dir = services.join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    let mut agents = HashMap::new();
    agents.insert(name.to_owned(), agent(name));
    let file = serde_yaml::to_string(
        &serde_yaml::to_value(HashMap::from([("agents".to_owned(), agents)])).unwrap(),
    )
    .unwrap();
    fs::write(agents_dir.join(format!("{name}.yaml")), file).unwrap();

    let config_dir = services.join("config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.yaml"),
        format!("includes:\n  - ../agents/{name}.yaml\n"),
    )
    .unwrap();
}

#[test]
fn delete_removes_agent_file_and_include() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "doomed");
    let agent_file = tmp.path().join("agents/doomed.yaml");
    assert!(agent_file.exists());

    let authoring = AgentConfigAuthoringService::new(tmp.path());
    delete_single_agent(&AgentName::new("doomed"), false, &authoring).unwrap();

    assert!(!agent_file.exists());
    let config = fs::read_to_string(tmp.path().join("config/config.yaml")).unwrap();
    assert!(!config.contains("doomed"));
}

#[test]
fn delete_reports_missing_agent_as_error() {
    let tmp = tempfile::tempdir().unwrap();
    let authoring = AgentConfigAuthoringService::new(tmp.path());

    let err = delete_single_agent(&AgentName::new("absent"), false, &authoring).unwrap_err();

    assert!(format!("{err:#}").contains("absent"));
}

#[test]
fn delete_failure_preserves_the_profile_include_for_repair() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "undeletable");
    let agent_path = tmp.path().join("agents/undeletable.yaml");
    fs::remove_file(&agent_path).unwrap();
    fs::create_dir(&agent_path).unwrap();
    fs::write(agent_path.join("keep"), "not an agent definition").unwrap();

    let authoring = AgentConfigAuthoringService::new(tmp.path());
    let error = delete_single_agent(&AgentName::new("undeletable"), false, &authoring)
        .expect_err("a directory cannot be removed through the agent-file authoring path");

    assert!(format!("{error:#}").contains("undeletable"), "{error:#}");
    assert!(
        agent_path.is_dir(),
        "the failed target remains available for repair"
    );
    let config = fs::read_to_string(tmp.path().join("config/config.yaml")).unwrap();
    assert!(
        config.contains("../agents/undeletable.yaml"),
        "a failed deletion must not orphan the still-present target by dropping its include: \
         {config}"
    );
}

#[test]
fn a_running_agent_is_refused_and_its_definition_kept() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "busy");
    let agent_file = tmp.path().join("agents/busy.yaml");
    let config_file = tmp.path().join("config/config.yaml");
    let original_agent = fs::read(&agent_file).unwrap();
    let original_config = fs::read(&config_file).unwrap();
    let authoring = AgentConfigAuthoringService::new(tmp.path());

    let error = delete_single_agent(&AgentName::new("busy"), true, &authoring)
        .expect_err("a running agent must not be deleted");
    assert!(
        matches!(error, AgentDeleteError::Running { .. }),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("infra services stop agent busy"),
        "{error}"
    );
    assert_eq!(fs::read(&agent_file).unwrap(), original_agent);
    assert_eq!(fs::read(&config_file).unwrap(), original_config);
}
