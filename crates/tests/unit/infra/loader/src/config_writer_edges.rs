//! Edge-case tests for `ConfigWriter`: round-trip of written agent files,
//! include removal (quoted/unquoted), the expected-file-miss scan fallback,
//! and malformed-file error propagation.

use systemprompt_loader::ConfigWriter;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};
use tempfile::TempDir;

fn test_agent(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_string(),
        port: 4000,
        endpoint: format!("http://localhost:4000/{name}"),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: Vec::new(),
        card: AgentCardConfig {
            protocol_version: "0.2.3".to_string(),
            name: Some(name.to_string()),
            display_name: format!("{name} Display"),
            description: format!("Desc {name}"),
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

#[test]
fn created_agent_round_trips_through_loader() {
    let temp = TempDir::new().expect("tempdir");
    let agent = test_agent("round_trip");

    let file = ConfigWriter::create_agent(&agent, temp.path()).expect("create");

    let found = ConfigWriter::find_agent_file("round_trip", temp.path())
        .expect("find ok")
        .expect("agent present");
    assert_eq!(found, file, "find must locate the file create just wrote");

    let content = std::fs::read_to_string(&file).expect("read");
    assert!(
        content.contains("round_trip Display"),
        "header display name"
    );
    assert!(content.contains("Desc round_trip"), "header description");
}

#[test]
fn delete_agent_strips_quoted_include_line() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");

    let config_content = "includes:\n  - \"../agents/quoted.yaml\"\n  - ../agents/keep.yaml\nagents: {}\nmcp_servers: {}\n";
    std::fs::write(config_dir.join("config.yaml"), config_content).expect("write config");

    let agent = test_agent("quoted");
    let agent_file = agents_dir.join("quoted.yaml");
    let yaml = format!(
        "agents:\n  quoted:\n    name: quoted\n    port: 4000\n    endpoint: http://localhost:4000/quoted\n    enabled: true\n    card:\n      protocolVersion: \"0.2.3\"\n      displayName: {}\n      description: d\n      version: 1.0.0\n      preferredTransport: JSONRPC\n      capabilities: {{streaming: true, pushNotifications: false, stateTransitionHistory: false}}\n      defaultInputModes: [text/plain]\n      defaultOutputModes: [text/plain]\n      skills: []\n      supportsAuthenticatedExtendedCard: false\n    metadata: {{}}\n",
        agent.card.display_name
    );
    std::fs::write(&agent_file, yaml).expect("write agent");

    ConfigWriter::delete_agent("quoted", temp.path()).expect("delete");

    assert!(!agent_file.exists(), "agent file removed");
    let updated = std::fs::read_to_string(config_dir.join("config.yaml")).expect("read config");
    assert!(
        !updated.contains("quoted.yaml"),
        "quoted include line must be stripped, got: {updated}"
    );
    assert!(
        updated.contains("keep.yaml"),
        "unrelated include lines must be preserved"
    );
}

#[test]
fn find_agent_file_falls_back_to_scan_when_expected_file_wrong_agent() {
    let temp = TempDir::new().expect("tempdir");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");

    let other = test_agent("other");
    ConfigWriter::create_agent(&other, temp.path()).expect("create other");
    std::fs::rename(
        agents_dir.join("other.yaml"),
        agents_dir.join("target.yaml"),
    )
    .expect("rename so expected filename mismatches agent key");

    let wanted = test_agent("target");
    std::fs::write(
        agents_dir.join("target.yaml"),
        {
            let _ = &wanted;
            "agents:\n  target:\n    name: target\n    port: 4000\n    endpoint: http://localhost:4000/target\n    enabled: true\n    card:\n      protocolVersion: \"0.2.3\"\n      displayName: T\n      description: d\n      version: 1.0.0\n      preferredTransport: JSONRPC\n      capabilities: {streaming: true, pushNotifications: false, stateTransitionHistory: false}\n      defaultInputModes: [text/plain]\n      defaultOutputModes: [text/plain]\n      skills: []\n      supportsAuthenticatedExtendedCard: false\n    metadata: {}\n"
        },
    )
    .expect("write target");

    std::fs::write(
        agents_dir.join("aaa-decoy.yaml"),
        "agents:\n  decoy:\n    name: decoy\n    port: 4001\n    endpoint: http://localhost:4001/decoy\n    enabled: true\n    card:\n      protocolVersion: \"0.2.3\"\n      displayName: D\n      description: d\n      version: 1.0.0\n      preferredTransport: JSONRPC\n      capabilities: {streaming: true, pushNotifications: false, stateTransitionHistory: false}\n      defaultInputModes: [text/plain]\n      defaultOutputModes: [text/plain]\n      skills: []\n      supportsAuthenticatedExtendedCard: false\n    metadata: {}\n",
    )
    .expect("write decoy");

    let found = ConfigWriter::find_agent_file("target", temp.path())
        .expect("find ok")
        .expect("agent present via scan");
    assert!(
        found.to_string_lossy().contains("target.yaml"),
        "scan must locate the file whose content holds the agent key"
    );
}

#[test]
fn find_agent_file_propagates_malformed_yaml_error() {
    let temp = TempDir::new().expect("tempdir");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");

    std::fs::write(
        agents_dir.join("looked_up.yaml"),
        "this: is: not: valid: : :",
    )
    .expect("write malformed");

    let err = ConfigWriter::find_agent_file("looked_up", temp.path())
        .expect_err("malformed agent file must surface a parse error");
    assert!(
        !err.to_string().is_empty(),
        "error message should be non-empty"
    );
}

#[test]
fn delete_agent_without_config_file_errors() {
    let temp = TempDir::new().expect("tempdir");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");

    let agent = test_agent("orphan");
    let yaml = "agents:\n  orphan:\n    name: orphan\n    port: 4000\n    endpoint: http://localhost:4000/orphan\n    enabled: true\n    card:\n      protocolVersion: \"0.2.3\"\n      displayName: O\n      description: d\n      version: 1.0.0\n      preferredTransport: JSONRPC\n      capabilities: {streaming: true, pushNotifications: false, stateTransitionHistory: false}\n      defaultInputModes: [text/plain]\n      defaultOutputModes: [text/plain]\n      skills: []\n      supportsAuthenticatedExtendedCard: false\n    metadata: {}\n";
    std::fs::write(agents_dir.join("orphan.yaml"), yaml).expect("write agent");
    let _ = &agent;

    let err = ConfigWriter::delete_agent("orphan", temp.path())
        .expect_err("missing config.yaml must error after file removal");
    assert!(!err.to_string().is_empty());
}

#[test]
fn create_agent_errors_when_agents_path_is_a_file() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(temp.path().join("agents"), "i am a file, not a directory")
        .expect("write blocking file");

    let agent = test_agent("blocked");
    let err = ConfigWriter::create_agent(&agent, temp.path())
        .expect_err("create_dir_all must fail when the agents path is an existing file");
    assert!(
        !err.to_string().is_empty(),
        "the create_dir_all IO failure must be surfaced"
    );
}

#[test]
fn update_agent_writes_through_content_scan_when_filename_differs() {
    let temp = TempDir::new().expect("tempdir");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");

    let yaml = "agents:\n  scanned:\n    name: scanned\n    port: 4000\n    endpoint: http://localhost:4000/scanned\n    enabled: true\n    card:\n      protocolVersion: \"0.2.3\"\n      displayName: S\n      description: original description\n      version: 1.0.0\n      preferredTransport: JSONRPC\n      capabilities: {streaming: true, pushNotifications: false, stateTransitionHistory: false}\n      defaultInputModes: [text/plain]\n      defaultOutputModes: [text/plain]\n      skills: []\n      supportsAuthenticatedExtendedCard: false\n    metadata: {}\n";
    let mismatched_file = agents_dir.join("zz-not-the-agent-name.yaml");
    std::fs::write(&mismatched_file, yaml).expect("write mismatched-name agent file");

    let mut updated = test_agent("scanned");
    updated.card.description = "updated via scan".to_string();
    ConfigWriter::update_agent("scanned", &updated, temp.path())
        .expect("update must locate the file via the content scan and rewrite it");

    let content = std::fs::read_to_string(&mismatched_file).expect("read rewritten file");
    assert!(
        content.contains("updated via scan"),
        "the scanned file must be rewritten in place, got: {content}"
    );
}
