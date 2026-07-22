//! Tests for single-agent deletion and process-stop resolution.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use systemprompt_agent::services::config_authoring::AgentConfigAuthoringService;
use systemprompt_cli::admin::agents::delete::{delete_single_agent, stop_agent_process};
use systemprompt_models::services::{
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
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
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

#[tokio::test]
async fn stop_without_orchestrator_or_port_assumes_stopped() {
    assert!(stop_agent_process("ghost", None, None).await);
}

#[tokio::test]
async fn stop_with_unoccupied_port_assumes_stopped() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    assert!(stop_agent_process("ghost", Some(port), None).await);
}

#[tokio::test]
async fn delete_removes_agent_file_and_include() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "doomed");
    let agent_file = tmp.path().join("agents/doomed.yaml");
    assert!(agent_file.exists());

    let authoring = AgentConfigAuthoringService::new(tmp.path());
    delete_single_agent("doomed", None, None, &authoring, false)
        .await
        .unwrap();

    assert!(!agent_file.exists());
    let config = fs::read_to_string(tmp.path().join("config/config.yaml")).unwrap();
    assert!(!config.contains("doomed"));
}

#[tokio::test]
async fn delete_reports_missing_agent_as_error() {
    let tmp = tempfile::tempdir().unwrap();
    let authoring = AgentConfigAuthoringService::new(tmp.path());

    let err = delete_single_agent("absent", None, None, &authoring, false)
        .await
        .unwrap_err();

    assert!(err.contains("absent"));
}
