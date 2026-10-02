//! Tests for single-agent deletion and the verified process-stop rule: delete
//! never kills a port holder it cannot prove is the agent's recorded process.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use systemprompt_agent::services::config_authoring::AgentConfigAuthoringService;
use systemprompt_cli::admin::agents::delete::{delete_single_agent, stop_verified_port_holder};
use systemprompt_identifiers::AgentName;
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

#[test]
fn nothing_recorded_and_no_port_is_stopped() {
    assert!(stop_verified_port_holder(
        &AgentName::new("ghost"),
        None,
        None
    ));
}

#[test]
fn a_recorded_process_without_a_port_to_verify_is_not_assumed_stopped() {
    assert!(!stop_verified_port_holder(
        &AgentName::new("ghost"),
        None,
        Some(424242)
    ));
}

#[test]
fn an_unoccupied_port_is_stopped() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    assert!(stop_verified_port_holder(
        &AgentName::new("ghost"),
        Some(port),
        None
    ));
}

#[test]
fn delete_removes_agent_file_and_include() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "doomed");
    let agent_file = tmp.path().join("agents/doomed.yaml");
    assert!(agent_file.exists());

    let authoring = AgentConfigAuthoringService::new(tmp.path());
    delete_single_agent(&AgentName::new("doomed"), true, &authoring, false).unwrap();

    assert!(!agent_file.exists());
    let config = fs::read_to_string(tmp.path().join("config/config.yaml")).unwrap();
    assert!(!config.contains("doomed"));
}

#[test]
fn delete_reports_missing_agent_as_error() {
    let tmp = tempfile::tempdir().unwrap();
    let authoring = AgentConfigAuthoringService::new(tmp.path());

    let err = delete_single_agent(&AgentName::new("absent"), true, &authoring, false).unwrap_err();

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
    let error = delete_single_agent(&AgentName::new("undeletable"), true, &authoring, false)
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
fn a_failed_stop_preserves_config_until_force_is_requested() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "force-owned");
    let agent_file = tmp.path().join("agents/force-owned.yaml");
    let config_file = tmp.path().join("config/config.yaml");
    let original_agent = fs::read(&agent_file).unwrap();
    let original_config = fs::read(&config_file).unwrap();
    let authoring = AgentConfigAuthoringService::new(tmp.path());

    let error = delete_single_agent(&AgentName::new("force-owned"), false, &authoring, false)
        .expect_err("a failed process stop must prevent ordinary deletion");
    assert!(
        error.to_string().contains("Use --force to delete anyway"),
        "{error}"
    );
    assert_eq!(fs::read(&agent_file).unwrap(), original_agent);
    assert_eq!(fs::read(&config_file).unwrap(), original_config);

    delete_single_agent(&AgentName::new("force-owned"), false, &authoring, true)
        .expect("force permits config deletion after the same stop failure");
    assert!(!agent_file.exists());
}

const OWNED_LISTENER_HELPER: &str = "commands::agents_delete_flow::owned_listener_helper";

#[test]
#[ignore = "re-executed by the verified port-holder tests"]
fn owned_listener_helper() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind owned listener");
    println!("OWNED_PORT={}", listener.local_addr().unwrap().port());
    std::io::stdout().flush().unwrap();
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    drop(listener);
}

struct OwnedListenerChild(std::process::Child);

impl Drop for OwnedListenerChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_owned_listener(agent: &str) -> (OwnedListenerChild, u32, u16) {
    use std::io::BufRead;
    use std::process::Stdio;

    let mut child = OwnedListenerChild(
        std::process::Command::new(std::env::current_exe().expect("unit-test binary path"))
            .args(["--exact", OWNED_LISTENER_HELPER, "--ignored", "--nocapture"])
            .env("SYSTEMPROMPT_SUBPROCESS", "1")
            .env("AGENT_NAME", agent)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn owned listener helper"),
    );
    let child_pid = child.0.id();
    let stdout = child.0.stdout.take().expect("helper stdout");
    let mut lines = std::io::BufReader::new(stdout).lines();
    let port = loop {
        let line = lines
            .next()
            .expect("helper reports its port before exiting")
            .expect("helper stdout line");
        if let Some((_, port)) = line.split_once("OWNED_PORT=") {
            break port.parse::<u16>().expect("numeric owned port");
        }
    };
    (child, child_pid, port)
}

// Why: agent delete used to kill whatever process held the agent's port.
#[test]
fn an_unverified_port_holder_is_never_killed() {
    use systemprompt_scheduler::ProcessCleanup;

    let (_child, child_pid, port) = spawn_owned_listener("owned-other");
    assert_eq!(ProcessCleanup::check_port(port), Some(child_pid));

    assert!(!stop_verified_port_holder(
        &AgentName::new("stranger"),
        Some(port),
        None
    ));
    assert!(!stop_verified_port_holder(
        &AgentName::new("stranger"),
        Some(port),
        Some(child_pid.wrapping_add(1))
    ));
    assert!(
        !stop_verified_port_holder(&AgentName::new("stranger"), Some(port), Some(child_pid)),
        "a recorded pid whose spawn markers name another agent must not be killed"
    );
    assert_eq!(
        ProcessCleanup::check_port(port),
        Some(child_pid),
        "a process that is not the agent's recorded pid must survive"
    );
}

#[test]
fn the_recorded_agent_process_is_stopped() {
    use systemprompt_scheduler::ProcessCleanup;

    let (mut child, child_pid, port) = spawn_owned_listener("owned-running");
    assert_eq!(ProcessCleanup::check_port(port), Some(child_pid));

    assert!(stop_verified_port_holder(
        &AgentName::new("owned-running"),
        Some(port),
        Some(child_pid)
    ));

    let status = child.0.wait().expect("reap owned listener helper");
    assert!(
        !status.success(),
        "the helper is terminated by the teardown path"
    );
}
