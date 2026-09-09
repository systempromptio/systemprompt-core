//! Launch validation, output bounding and cleanup for evaluator containers.
//!
//! The runtime tests stand a small executable in for `docker`. That keeps the
//! real spawn, poll, cancel and drop paths under test — including the bounded
//! cleanup wait — without requiring a container runtime on the machine.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};
use systemprompt_evaluation::experiments::ClientKind;
use systemprompt_evaluation::experiments::execution::ExecutionLimits;
use systemprompt_identifiers::ModelId;
use systemprompt_scheduler::services::evaluator::client::NativeClient;
use systemprompt_scheduler::services::evaluator::container::{ContainerExecution, ContainerLaunch};

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn launch(
    docker: PathBuf,
    directory: PathBuf,
) -> systemprompt_scheduler::SchedulerResult<ContainerLaunch> {
    ContainerLaunch::builder(docker, directory)
        .image(DIGEST.to_owned())
        .network("eval-net-1".to_owned())
        .name("eval-run-1".to_owned())
        .build()
}

fn client(limits: ExecutionLimits) -> NativeClient {
    NativeClient::builder(ClientKind::ClaudeCode, ModelId::new("claude-opus-5"))
        .limits(limits)
        .build()
        .expect("limits used by these tests are inside the supported envelope")
}

fn fake_docker(directory: &Path, cleanup_body: &str) -> PathBuf {
    let path = directory.join("fake-docker");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"rm\" ]; then\n{cleanup_body}\nfi\necho started\nexit 0\n"
        ),
    )
    .expect("the stand-in docker binary must be writable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("the stand-in docker binary must be executable");
    }
    path
}

fn started(cleanup_body: &str, limits: ExecutionLimits) -> (tempfile::TempDir, ContainerExecution) {
    let workspace = tempfile::tempdir().expect("a workspace directory must be creatable");
    let docker = fake_docker(workspace.path(), cleanup_body);
    let execution = launch(docker, workspace.path().to_path_buf())
        .expect("the launch spec is valid")
        .start(&client(limits), "prompt")
        .expect("the stand-in docker binary must spawn");
    std::thread::sleep(std::time::Duration::from_millis(400));
    (workspace, execution)
}

#[test]
fn a_valid_specification_builds() {
    let workspace = tempfile::tempdir().expect("workspace");
    assert!(
        launch(
            PathBuf::from("/usr/bin/docker"),
            workspace.path().to_path_buf()
        )
        .is_ok(),
        "an absolute docker path, pinned digest, private network and eval- name must build"
    );
}

#[test]
fn the_image_must_be_a_pinned_digest() {
    let workspace = tempfile::tempdir().expect("workspace");
    for image in [
        "evaluator:latest",
        "sha256:short",
        "sha256:zzzz56789abcdef0123456789abcdef0123456789abcdef0123456789abcdefg",
    ] {
        assert!(
            ContainerLaunch::builder(
                PathBuf::from("/usr/bin/docker"),
                workspace.path().to_path_buf()
            )
            .image(image.to_owned())
            .network("eval-net-1".to_owned())
            .name("eval-run-1".to_owned())
            .build()
            .is_err(),
            "{image} is not a 64-character sha256 digest and must be refused"
        );
    }
}

#[test]
fn every_required_field_must_be_supplied() {
    let workspace = tempfile::tempdir().expect("workspace");
    let base = || {
        ContainerLaunch::builder(
            PathBuf::from("/usr/bin/docker"),
            workspace.path().to_path_buf(),
        )
    };

    assert!(
        base()
            .network("eval-net-1".to_owned())
            .name("eval-run-1".to_owned())
            .build()
            .is_err(),
        "a launch without a pinned image must be refused"
    );
    assert!(
        base()
            .image(DIGEST.to_owned())
            .name("eval-run-1".to_owned())
            .build()
            .is_err(),
        "a launch without a private network must be refused"
    );
    assert!(
        base()
            .image(DIGEST.to_owned())
            .network("eval-net-1".to_owned())
            .build()
            .is_err(),
        "a launch without an execution name must be refused"
    );
}

#[test]
fn shared_networks_are_refused() {
    let workspace = tempfile::tempdir().expect("workspace");
    for network in ["host", "bridge", "default", "none", "eval net", ""] {
        assert!(
            ContainerLaunch::builder(
                PathBuf::from("/usr/bin/docker"),
                workspace.path().to_path_buf()
            )
            .image(DIGEST.to_owned())
            .network(network.to_owned())
            .name("eval-run-1".to_owned())
            .build()
            .is_err(),
            "network {network:?} would break evaluation isolation and must be refused"
        );
    }
}

#[test]
fn execution_names_must_be_namespaced_and_shell_safe() {
    let workspace = tempfile::tempdir().expect("workspace");
    for name in [
        "run-1",
        "eval-run 1",
        "eval-run;rm",
        &format!("eval-{}", "a".repeat(200)),
    ] {
        assert!(
            ContainerLaunch::builder(
                PathBuf::from("/usr/bin/docker"),
                workspace.path().to_path_buf()
            )
            .image(DIGEST.to_owned())
            .network("eval-net-1".to_owned())
            .name(name.to_owned())
            .build()
            .is_err(),
            "name {name:?} must be refused"
        );
    }
}

#[test]
fn relative_paths_and_mount_separators_are_refused() {
    let workspace = tempfile::tempdir().expect("workspace");
    assert!(
        launch(PathBuf::from("docker"), workspace.path().to_path_buf()).is_err(),
        "a relative docker path must be refused"
    );
    assert!(
        launch(
            PathBuf::from("/usr/bin/docker"),
            PathBuf::from("relative/workspace")
        )
        .is_err(),
        "a relative workspace must be refused"
    );
    assert!(
        launch(
            PathBuf::from("/usr/bin/docker"),
            PathBuf::from("/tmp/eval,workspace"),
        )
        .is_err(),
        "a comma would split the bind-mount specification and must be refused"
    );
}

#[test]
fn starting_twice_in_one_workspace_is_refused() {
    let workspace = tempfile::tempdir().expect("workspace");
    let docker = fake_docker(workspace.path(), "exit 0");
    let spec = launch(docker, workspace.path().to_path_buf()).expect("valid spec");
    let native = client(ExecutionLimits::default());

    let first = spec
        .start(&native, "prompt")
        .expect("the first start spawns");
    assert!(
        spec.start(&native, "prompt").is_err(),
        "reusing a workspace would overwrite an existing event log and must fail"
    );
    drop(first);
}

#[test]
fn polling_reports_the_client_exit_status() {
    let (_workspace, mut execution) = started("exit 0", ExecutionLimits::default());

    let status = execution
        .poll()
        .expect("polling inside the limits must not error")
        .expect("the stand-in client has already exited");

    assert!(status.success(), "the stand-in client exits zero");
}

#[test]
fn exceeding_the_output_budget_cancels_the_run() {
    let (_workspace, mut execution) = started(
        "exit 0",
        ExecutionLimits {
            max_artifact_bytes: 1,
            ..ExecutionLimits::default()
        },
    );

    let error = execution
        .poll()
        .expect_err("output beyond the budget must fail the execution");

    assert!(
        error.to_string().contains("execution time or output limit"),
        "the failure must name the breached limit, got: {error}"
    );
}

#[test]
fn exceeding_the_active_timeout_cancels_the_run() {
    let (_workspace, mut execution) = started(
        "exit 0",
        ExecutionLimits {
            active_timeout_seconds: 1,
            ..ExecutionLimits::default()
        },
    );
    std::thread::sleep(std::time::Duration::from_millis(2000));

    let error = execution
        .poll()
        .expect_err("an execution past its active timeout must fail");

    assert!(
        error.to_string().contains("execution time or output limit"),
        "the failure must name the breached limit, got: {error}"
    );
}

#[test]
fn an_unacknowledged_cleanup_is_an_error() {
    let (_workspace, mut execution) = started(
        "exit 1",
        ExecutionLimits {
            max_artifact_bytes: 1,
            ..ExecutionLimits::default()
        },
    );

    let error = execution
        .poll()
        .expect_err("a refused cleanup must surface, never be swallowed");

    assert!(
        error.to_string().contains("cleanup was not acknowledged"),
        "a docker rm that fails must be reported as an unacknowledged cleanup, got: {error}"
    );
}

#[test]
fn a_hanging_cleanup_is_bounded_and_killed() {
    let (_workspace, mut execution) = started(
        "exec sleep 120",
        ExecutionLimits {
            max_artifact_bytes: 1,
            ..ExecutionLimits::default()
        },
    );
    let started_at = std::time::Instant::now();

    let error = execution
        .poll()
        .expect_err("a cleanup that never returns must not hang the scheduler");

    assert!(
        error.to_string().contains("timed out"),
        "the bounded wait must report a timeout, got: {error}"
    );
    assert!(
        started_at.elapsed() < std::time::Duration::from_secs(30),
        "the cleanup wait is bounded to ten seconds, took {:?}",
        started_at.elapsed()
    );
}
