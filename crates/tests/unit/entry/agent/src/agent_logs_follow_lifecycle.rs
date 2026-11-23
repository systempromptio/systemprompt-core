#![cfg(unix)]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use systemprompt_test_fixtures::bounded_output;

use systemprompt_agent_entry::cli::agents;

use crate::common::{agents_host, json_cli, logs_dir_in_root, parse_agents, rendered, with_boot};

const HELPER: &str = "agent_logs_follow_lifecycle::agent_logs_follow_helper";

#[tokio::test]
#[ignore = "re-executed by follow_uses_the_resolved_agent_log_and_surfaces_tail_failure"]
async fn agent_logs_follow_helper() {
    let fixture = tempfile::tempdir().expect("agent log fixture");
    let logs_dir = logs_dir_in_root(&with_boot(json_cli())).await;
    let logs = logs_dir.path().to_path_buf();
    let bin = fixture.path().join("bin");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let log = logs.join("agent-orion.log");
    std::fs::write(&log, "INFO orion ready\n").unwrap();
    let capture = fixture.path().join("tail-args");
    let tail = bin.join("tail");
    std::fs::write(
        &tail,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$TAIL_CAPTURE\"\nexit \"${TAIL_EXIT:-0}\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&tail).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&tail, permissions).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("TAIL_CAPTURE", &capture);
        std::env::remove_var("TAIL_EXIT");
    }

    let context = with_boot(json_cli());
    let host = agents_host(&context);
    println!("BEGIN_FOLLOW");
    rendered(
        &host,
        agents::execute(
            parse_agents(&[
                "logs",
                "orion",
                "--follow",
                "--logs-dir",
                logs.to_str().unwrap(),
            ]),
            &host,
        )
        .await,
    )
    .expect("successful tail follow renders an output artifact");
    println!("END_FOLLOW");

    let invoked = std::fs::read_to_string(&capture).expect("tail invocation capture");
    assert_eq!(invoked, format!("-f\n{}\n", log.display()));

    unsafe { std::env::set_var("TAIL_EXIT", "9") };
    let error = agents::execute(
        parse_agents(&[
            "logs",
            "orion",
            "--follow",
            "--logs-dir",
            logs.to_str().unwrap(),
        ]),
        &host,
    )
    .await
    .expect_err("a failed tail process must be reported");
    let rendered = format!("{error:#}");
    assert!(rendered.contains("Failed to get agent logs"), "{rendered}");
    assert!(
        rendered.contains("tail -f exited with non-zero status"),
        "{rendered}"
    );
}

#[test]
fn follow_uses_the_resolved_agent_log_and_surfaces_tail_failure() {
    let mut command = Command::new(std::env::current_exe().expect("unit test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command, "agent logs follow lifecycle");
    assert!(
        output.status.success(),
        "follow helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("follow output UTF-8");
    let json = stdout
        .split_once("BEGIN_FOLLOW")
        .and_then(|(_, tail)| tail.split_once("END_FOLLOW"))
        .map(|(value, _)| value.trim())
        .expect("follow output markers");
    let artifact: serde_json::Value = serde_json::from_str(json).expect("strict follow JSON");
    assert_eq!(artifact["title"], "Agent Logs", "{artifact}");
    let sections = artifact["sections"].as_array().expect("follow sections");
    let content = |heading: &str| {
        &sections
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
    };
    assert_eq!(content("agent"), "orion", "{artifact}");
    assert_eq!(content("source"), "disk", "{artifact}");
    assert_eq!(content("logs"), &serde_json::json!([]), "{artifact}");
}
