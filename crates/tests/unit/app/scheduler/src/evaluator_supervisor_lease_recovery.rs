//! Supervisor restart ordering against real leases, capabilities and spend.
//! The owned Docker fixture models only object discovery/removal, never a CLI
//! execution or native admission proof.
#![cfg(unix)]

use super::Fixture;
use std::path::{Path, PathBuf};
use systemprompt_scheduler::services::evaluator::supervisor::{
    EvaluatorSupervisor, EvaluatorSupervisorConfig,
};

struct DockerObjects {
    directory: tempfile::TempDir,
    executable: PathBuf,
}
impl DockerObjects {
    fn new(execution: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("docker");
        for name in ["container", "network"] {
            std::fs::write(directory.path().join(name), b"owned").unwrap();
        }
        std::fs::write(
            &executable,
            format!(
                r#"#!/bin/sh
set -eu
root='{root}'
printf '%s\n' "$*" >> "$root/calls"
printf '%s\n' "$DOCKER_CONFIG" >> "$root/configs"
test "$HOME" = "$DOCKER_CONFIG"
case "$1 ${{2:-}}" in
  'ps -aq') if [ -f "$root/container" ]; then echo owned-container; fi ;;
  'network ls') if [ -f "$root/network" ]; then echo owned-network; fi ;;
  'inspect --format'|'network inspect') echo '{execution}' ;;
  'rm --force') [ ! -f "$root/refuse-removal" ] || exit 19; rm "$root/container" ;;
  'network rm') [ ! -f "$root/refuse-removal" ] || exit 19; rm "$root/network" ;;
  *) exit 97 ;;
esac
"#,
                root = directory.path().display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            directory,
            executable,
        }
    }
    fn supervisor(&self, f: &Fixture) -> EvaluatorSupervisor {
        EvaluatorSupervisor::new(
            &f.repositories,
            EvaluatorSupervisorConfig {
                docker: self.executable.clone(),
                workspace_root: f.root.path().to_owned(),
                environment: "terminal-fixture".to_owned(),
                client_image: format!("client@sha256:{}", "a".repeat(64)),
                relay_image: format!("relay@sha256:{}", "b".repeat(64)),
                relay_control_network: "fixture-control".to_owned(),
                relay_upstream: "http://127.0.0.1:1".to_owned(),
            },
        )
        .unwrap()
    }
    async fn run(&self, f: &Fixture) -> Result<bool, String> {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.supervisor(f).run_once(&f.owner, &f.lease.worker_id),
        )
        .await
        .expect("bounded local supervisor protocol")
        .map_err(|e| e.to_string())
    }
    fn assert_configs_cleaned(&self) {
        for config in std::fs::read_to_string(self.directory.path().join("configs"))
            .unwrap()
            .lines()
        {
            assert!(
                !Path::new(config).exists(),
                "temporary Docker credential config must not survive"
            );
        }
    }
    fn calls(&self) -> String {
        std::fs::read_to_string(self.directory.path().join("calls")).unwrap()
    }
    fn objects_exist(&self) -> bool {
        self.directory.path().join("container").exists()
            && self.directory.path().join("network").exists()
    }
}

#[tokio::test]
async fn supervisor_restart_preserves_live_owned_objects_and_execution_access() {
    let f = Fixture::new().await;
    let access = f
        .repositories
        .capabilities
        .issue(&f.owner, &f.lease)
        .await
        .unwrap();
    let docker = DockerObjects::new(f.lease.execution_id.as_str());
    assert!(!docker.run(&f).await.unwrap());
    assert!(docker.objects_exist());
    assert!(
        !docker
            .calls()
            .lines()
            .any(|line| line.starts_with("rm ") || line.starts_with("network rm"))
    );
    assert_eq!(f.state().await["status"], "running");
    assert_eq!(f.budget().await, (400, 0));
    assert!(
        f.repositories
            .capabilities
            .authenticate(access.expose_token(), "terminal-fixture")
            .await
            .is_ok()
    );
    docker.assert_configs_cleaned();
    f.cleanup_rows().await;
}

#[tokio::test]
async fn expired_lease_is_invalidated_and_uncertain_spend_retained_even_when_cleanup_requires_retry()
 {
    let f = Fixture::new().await;
    let access = f
        .repositories
        .capabilities
        .issue(&f.owner, &f.lease)
        .await
        .unwrap();
    let docker = DockerObjects::new(f.lease.execution_id.as_str());
    sqlx::query(
        "UPDATE eval_executions SET lease_expires_at=NOW()-INTERVAL '1 second' WHERE id=$1",
    )
    .bind(f.lease.execution_id.as_str())
    .execute(&f.pg)
    .await
    .unwrap();
    std::fs::write(docker.directory.path().join("refuse-removal"), b"retry").unwrap();
    assert!(
        docker
            .run(&f)
            .await
            .unwrap_err()
            .contains("cleanup was not acknowledged")
    );
    let first = f.state().await;
    assert_eq!(first["status"], "error");
    assert_eq!(first["cleanup"]["status"], "retrying");
    assert_eq!(first["evidence"], 0);
    assert_eq!(f.budget().await, (0, 400));
    assert!(docker.objects_exist());
    assert!(
        f.repositories
            .capabilities
            .authenticate(access.expose_token(), "terminal-fixture")
            .await
            .is_err()
    );
    std::fs::remove_file(docker.directory.path().join("refuse-removal")).unwrap();
    assert!(!docker.run(&f).await.unwrap());
    assert!(!docker.directory.path().join("container").exists());
    assert!(!docker.directory.path().join("network").exists());
    assert_eq!(
        f.budget().await,
        (0, 400),
        "cleanup retry never refunds uncertain spend"
    );
    assert_eq!(f.state().await["status"], "error");
    assert!(
        !docker.run(&f).await.unwrap(),
        "second restart remains idle"
    );
    assert_eq!(f.budget().await, (0, 400));
    docker.assert_configs_cleaned();
    f.cleanup_rows().await;
}

#[tokio::test]
async fn expired_deadline_with_live_lease_cannot_keep_objects_or_export_success_on_restart() {
    let f = Fixture::new().await;
    let docker = DockerObjects::new(f.lease.execution_id.as_str());
    sqlx::query("UPDATE eval_executions SET deadline_at=NOW()-INTERVAL '1 second' WHERE id=$1")
        .bind(f.lease.execution_id.as_str())
        .execute(&f.pg)
        .await
        .unwrap();
    assert!(!docker.run(&f).await.unwrap());
    assert_eq!(f.state().await["status"], "error");
    assert_eq!(f.state().await["evidence"], 0);
    assert!(!docker.objects_exist());
    assert!(!docker.run(&f).await.unwrap());
    assert_eq!(
        f.budget().await,
        (0, 400),
        "subsequent restart conservatively settles the expired request bound"
    );
    assert!(
        f.repositories
            .capabilities
            .issue(&f.owner, &f.lease)
            .await
            .is_err()
    );
    docker.assert_configs_cleaned();
    f.cleanup_rows().await;
}

#[tokio::test]
async fn owned_orphan_with_foreign_live_execution_label_cannot_change_other_owner_state() {
    let local = Fixture::new().await;
    let foreign = Fixture::new().await;
    let docker = DockerObjects::new(foreign.lease.execution_id.as_str());
    assert!(!docker.run(&local).await.unwrap());
    assert!(
        !docker.objects_exist(),
        "foreign DB identity cannot shield an object returned in this owner's listing"
    );
    let calls = docker.calls();
    assert!(calls.contains(&format!(
        "label=systemprompt.evaluator.owner={}",
        local.owner
    )));
    assert!(!calls.contains(&format!(
        "label=systemprompt.evaluator.owner={}",
        foreign.owner
    )));
    for f in [&local, &foreign] {
        assert_eq!(f.state().await["status"], "running");
        assert_eq!(f.budget().await, (400, 0));
    }
    docker.assert_configs_cleaned();
    local.cleanup_rows().await;
    foreign.cleanup_rows().await;
}
