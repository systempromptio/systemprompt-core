//! `cloud profile` local tenant setup drives `docker compose` and the
//! migration prompt; these run it against stub executables in a sandbox.

use systemprompt_cli::ScriptedPrompter;
use systemprompt_loader::subprocess::ForegroundChildren;

use crate::process_sandbox::Sandbox;

async fn local_setup(sandbox: &Sandbox, answers: &[&str]) -> anyhow::Result<()> {
    let compose = sandbox
        .root
        .path()
        .join(".systemprompt/docker/fixture.yaml");
    std::fs::write(compose, "services: {}\n").unwrap();
    systemprompt_cloud_entry::cli::profile::handle_local_tenant_setup(
        &ScriptedPrompter::new(answers.iter().copied()),
        "postgres://unused:unused@127.0.0.1:1/unused",
        "fixture",
        &sandbox.root.path().join("profile.yaml"),
        &ForegroundChildren::default(),
    )
    .await
}
#[tokio::test]
async fn coverage_local_setup_declining_docker_does_not_execute_it() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf called > calls; exit 1");
    local_setup(&sandbox, &["no"]).await.unwrap();
    assert!(!sandbox.root.path().join("calls").exists());
}
#[tokio::test]
async fn coverage_local_setup_failed_docker_start_does_not_ask_for_migrations() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf '%s\\n' \"$*\" > calls; exit 1");
    local_setup(&sandbox, &["yes"]).await.unwrap();
    let calls = sandbox.calls();
    assert!(calls.contains("compose -p fixture -f"));
    assert!(calls.trim().ends_with("up -d"));
}
#[tokio::test]
async fn coverage_local_setup_missing_docker_has_actionable_error() {
    let sandbox = Sandbox::new();
    let err = local_setup(&sandbox, &["yes"]).await.unwrap_err();
    assert!(err.to_string().contains("Failed to execute docker compose"));
}
#[tokio::test]
async fn coverage_local_setup_waits_for_health_before_offering_migrations() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf '%s\\n' \"$*\" >> calls; case \"$*\" in *' up -d') exit 0;; *) printf healthy;; esac");
    local_setup(&sandbox, &["yes", "no"]).await.unwrap();
    let calls = sandbox.calls();
    assert_eq!(calls.lines().count(), 2, "{calls}");
    assert!(calls.contains("ps"));
}
#[tokio::test]
async fn coverage_local_setup_health_probe_failure_does_not_offer_migrations() {
    let sandbox = Sandbox::new();
    sandbox.executable("docker", "printf '%s\\n' \"$*\" >> calls; case \"$*\" in *' up -d') /bin/rm -- \"$0\"; exit 0;; *) exit 3;; esac");
    local_setup(&sandbox, &["yes"]).await.unwrap();
    assert_eq!(sandbox.calls().lines().count(), 1);
}
