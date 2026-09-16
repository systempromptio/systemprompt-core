use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use systemprompt_scheduler::services::evaluator::container::ContainerLaunch;

fn failed_probe(cleanup_exit: u8) -> (tempfile::TempDir, String) {
    let workspace = tempfile::tempdir().expect("probe workspace");
    let executable = workspace.path().join("fake-docker");
    let script = format!(
        "#!/bin/sh\nroot=${{0%/*}}\nif [ \"$1\" = rm ]; then\n  printf removed > \"$root/cleanup\"\n  exit {cleanup_exit}\nfi\nprintf '%s\\n' \"$DOCKER_HOST\" \"$DOCKER_CONFIG\" \"$HOME\" \"${{DOCKER_AUTH_CONFIG-absent}}\" \"${{ANTHROPIC_API_KEY-absent}}\" > \"$root/environment\"\necho $$ > \"$root/child-pid\"\nrm \"$root/client-pin-failure.stdout\"\nexec sleep 30\n"
    );
    std::fs::write(&executable, script).expect("probe fixture");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
        .expect("executable fixture");
    let launch = ContainerLaunch::builder(executable, workspace.path().to_path_buf())
        .image(format!("fixture@sha256:{}", "a".repeat(64)))
        .network("eval-net-fixture".to_owned())
        .name("eval-probe-fixture".to_owned())
        .ownership("owner-fixture", "execution-fixture")
        .build()
        .expect("valid native probe specification");
    let error = temp_env::with_vars(
        [
            ("DOCKER_HOST", Some("unix:///fixture/docker.sock")),
            ("DOCKER_CONFIG", Some("/fixture/ambient-credentials")),
            ("DOCKER_AUTH_CONFIG", Some("fixture-registry-secret")),
            ("ANTHROPIC_API_KEY", Some("fixture-provider-secret")),
        ],
        || {
            launch
                .probe_client("failure", "/usr/bin/true", &[] as &[OsString])
                .expect_err("missing output metadata must fail the probe")
                .to_string()
        },
    );
    (workspace, error)
}

fn assert_probe_cleaned(workspace: &Path) {
    assert_eq!(
        std::fs::read_to_string(workspace.join("cleanup")).unwrap(),
        "removed"
    );
    let pid = std::fs::read_to_string(workspace.join("child-pid")).expect("child PID");
    assert!(
        !Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("process probe")
            .success(),
        "the Docker CLI child must be reaped even when probe IO fails"
    );
    let environment = std::fs::read_to_string(workspace.join("environment")).unwrap();
    let lines: Vec<_> = environment.lines().collect();
    assert_eq!(lines[0], "unix:///fixture/docker.sock");
    assert_ne!(lines[1], "/fixture/ambient-credentials");
    assert_eq!(lines[1], lines[2]);
    assert!(
        !Path::new(lines[1]).exists(),
        "isolated Docker configuration is removed"
    );
    assert_eq!(&lines[3..], &["absent", "absent"]);
}

#[test]
fn probe_metadata_failure_still_removes_the_container_and_reaps_the_cli() {
    let (workspace, error) = failed_probe(0);
    assert!(error.contains("probe cleanup confirmed"), "{error}");
    assert_probe_cleaned(workspace.path());
}

#[test]
fn probe_metadata_and_cleanup_failures_are_both_reported_while_the_cli_is_reaped() {
    let (workspace, error) = failed_probe(7);
    assert!(error.contains("probe cleanup failed"), "{error}");
    assert!(
        error.contains("container cleanup was not acknowledged"),
        "{error}"
    );
    assert_probe_cleaned(workspace.path());
}
