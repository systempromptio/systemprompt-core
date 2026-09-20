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

#[tokio::test]
async fn delete_failure_preserves_the_profile_include_for_repair() {
    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "undeletable");
    let agent_path = tmp.path().join("agents/undeletable.yaml");
    fs::remove_file(&agent_path).unwrap();
    fs::create_dir(&agent_path).unwrap();
    fs::write(agent_path.join("keep"), "not an agent definition").unwrap();

    let authoring = AgentConfigAuthoringService::new(tmp.path());
    let error = delete_single_agent("undeletable", None, None, &authoring, false)
        .await
        .expect_err("a directory cannot be removed through the agent-file authoring path");

    assert!(error.contains("undeletable"), "{error}");
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

const OWNED_LISTENER_HELPER: &str = "commands::agents_delete_flow::owned_listener_helper";

#[test]
#[ignore = "re-executed by delete_stops_owned_listener_before_removing_configuration"]
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

#[tokio::test]
async fn delete_stops_owned_listener_before_removing_configuration() {
    use std::io::BufRead;
    use std::process::Stdio;
    use systemprompt_scheduler::ProcessCleanup;

    let mut child = OwnedListenerChild(
        std::process::Command::new(std::env::current_exe().expect("unit-test binary path"))
            .args(["--exact", OWNED_LISTENER_HELPER, "--ignored", "--nocapture"])
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

    assert_eq!(
        ProcessCleanup::check_port(port),
        Some(child_pid),
        "the deletion target must be the exact child process owned by this test"
    );

    let tmp = tempfile::tempdir().unwrap();
    write_agent(tmp.path(), "owned-running");
    let authoring = AgentConfigAuthoringService::new(tmp.path());
    delete_single_agent("owned-running", Some(port), None, &authoring, false)
        .await
        .expect("stop the owned listener and delete its agent configuration");

    assert!(
        ProcessCleanup::check_port(port).is_none(),
        "the owned listener must be stopped"
    );
    assert!(
        !tmp.path().join("agents/owned-running.yaml").exists(),
        "the agent definition must be removed after teardown"
    );
    let config = fs::read_to_string(tmp.path().join("config/config.yaml")).unwrap();
    assert!(
        !config.contains("owned-running"),
        "the profile include must be removed after teardown: {config}"
    );

    let status = child.0.wait().expect("reap owned listener helper");
    assert!(
        !status.success(),
        "the helper is terminated by the teardown path"
    );
}
#[cfg(unix)]
const FAILED_STOP_HELPER: &str = "commands::agents_delete_flow::failed_stop_force_contract_helper";

#[cfg(unix)]
#[test]
#[ignore = "re-executed by failed_stop_preserves_config_until_force_is_requested"]
fn failed_stop_force_contract_helper() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build helper runtime");
    runtime.block_on(async {
        let tmp = tempfile::tempdir().expect("owned authoring root");
        write_agent(tmp.path(), "force-owned");
        let control_file = tmp.path().join("agents/control-owned.yaml");
        let mut control_agents = HashMap::new();
        control_agents.insert("control-owned".to_owned(), agent("control-owned"));
        let control_yaml = serde_yaml::to_string(
            &serde_yaml::to_value(HashMap::from([("agents".to_owned(), control_agents)]))
                .expect("serialize control agent value"),
        )
        .expect("serialize control agent document");
        fs::write(&control_file, control_yaml).expect("write control agent");
        let agent_file = tmp.path().join("agents/force-owned.yaml");
        let config_file = tmp.path().join("config/config.yaml");
        fs::write(
            &config_file,
            "includes:\n  - ../agents/force-owned.yaml\n  - ../agents/control-owned.yaml\n",
        )
        .expect("write two-agent include list");
        let original_agent = fs::read(&agent_file).expect("original agent bytes");
        let original_config = fs::read(&config_file).expect("original config bytes");
        let original_control = fs::read(&control_file).expect("original control bytes");
        let authoring = AgentConfigAuthoringService::new(tmp.path());

        let error = delete_single_agent("force-owned", Some(9001), None, &authoring, false)
            .await
            .expect_err("a failed process stop must prevent ordinary deletion");
        assert!(
            error.contains("Use --force to delete anyway"),
            "the refusal must explain the explicit recovery option: {error}"
        );
        assert_eq!(
            fs::read(&agent_file).expect("preserved agent bytes"),
            original_agent,
            "the authored agent must remain byte-identical after a failed stop"
        );
        assert_eq!(
            fs::read(&config_file).expect("preserved config bytes"),
            original_config,
            "the include list must remain byte-identical after a failed stop"
        );
        assert_eq!(
            fs::read(&control_file).expect("preserved control bytes"),
            original_control,
            "the unrelated agent must remain byte-identical"
        );

        delete_single_agent("force-owned", Some(9001), None, &authoring, true)
            .await
            .expect("force permits config deletion after the same stop failure");
        assert!(
            !agent_file.exists(),
            "force must remove the agent definition"
        );
        let deleted = fs::read_to_string(&config_file).expect("updated profile includes");
        assert!(
            !deleted.contains("force-owned"),
            "force must remove the profile include: {deleted}"
        );
        assert!(
            deleted.contains("../agents/control-owned.yaml"),
            "force must preserve unrelated profile includes: {deleted}"
        );
        assert_eq!(
            fs::read(&control_file).expect("control agent after force"),
            original_control,
            "force must preserve the unrelated agent definition"
        );
        println!("FORCE_CONTRACT_VERIFIED");
    });
}

#[cfg(unix)]
#[test]
fn failed_stop_preserves_config_until_force_is_requested() {
    use std::io::{Read, Seek, SeekFrom};
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let shim = tempfile::tempdir().expect("owned executable directory");
    let lsof = shim.path().join("lsof");
    let invocation_log = shim.path().join("lsof-invocations");
    fs::write(
        &lsof,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$LSOF_INVOCATIONS\"\nprintf '0\\n'\n",
    )
    .expect("write owned lsof shim");
    let mut permissions = fs::metadata(&lsof)
        .expect("lsof shim metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&lsof, permissions).expect("make lsof shim executable");

    let stdout = tempfile::tempfile().expect("owned stdout capture");
    let stderr = tempfile::tempfile().expect("owned stderr capture");
    let mut path_entries = vec![shim.path().to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }
    let child_path = std::env::join_paths(path_entries).expect("construct helper PATH");
    let mut child = Command::new(std::env::current_exe().expect("unit-test binary path"))
        .args(["--exact", FAILED_STOP_HELPER, "--ignored", "--nocapture"])
        .env("PATH", child_path)
        .env("LSOF_INVOCATIONS", &invocation_log)
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            stdout.try_clone().expect("clone stdout capture"),
        ))
        .stderr(Stdio::from(
            stderr.try_clone().expect("clone stderr capture"),
        ))
        .spawn()
        .expect("spawn isolated force-contract helper");

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll helper") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("force-contract helper exceeded twenty seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let read_capture = |mut file: std::fs::File| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut text = String::new();
        file.read_to_string(&mut text).expect("read capture");
        text
    };
    let captured_stdout = read_capture(stdout);
    let captured_stderr = read_capture(stderr);
    assert!(
        status.success(),
        "isolated force-contract helper failed; stdout={captured_stdout:?}; stderr={captured_stderr:?}"
    );
    assert!(
        captured_stdout.contains("FORCE_CONTRACT_VERIFIED"),
        "helper must reach both durable assertions: {captured_stdout:?}"
    );
    let invocations = fs::read_to_string(invocation_log).expect("recorded lsof invocations");
    assert_eq!(
        invocations.lines().collect::<Vec<_>>(),
        vec!["-ti :9001", "-ti :9001", "-ti :9001", "-ti :9001"],
        "both delete attempts must reach the intended PID-zero stop failure"
    );
}
