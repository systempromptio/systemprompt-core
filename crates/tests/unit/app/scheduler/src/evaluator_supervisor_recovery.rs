//! Real supervisor preparation/reconciliation against PostgreSQL and an owned
//! Docker protocol fixture. No native target is enabled and no model runs.
#![cfg(unix)]

use std::path::PathBuf;
use systemprompt_evaluation::repository::experiments::{EvaluationRepositories, WorkerRecord};
use systemprompt_identifiers::{EvalWorkerId, UserId};
use systemprompt_scheduler::services::evaluator::supervisor::{
    EvaluatorSupervisor, EvaluatorSupervisorConfig,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

struct Fixture {
    root: tempfile::TempDir,
    pg: sqlx::PgPool,
    owner: UserId,
    worker: WorkerRecord,
    repositories: EvaluationRepositories,
}
impl Fixture {
    async fn new() -> Self {
        let bootstrap = ensure_test_bootstrap();
        let db = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("database required");
        let pg = db.write_pool_arc().expect("write pool").as_ref().clone();
        let owner = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row(&db, &owner, &format!("{owner}@supervisor.test"))
            .await
            .expect("owner");
        let repositories = systemprompt_test_fixtures::fixture_evaluation_repositories(&db)
            .expect("evaluation repositories");
        let credential = repositories
            .workers
            .create(&owner, "fixture", "supervisor")
            .await
            .expect("worker");
        let worker = repositories
            .workers
            .authenticate(credential.expose_token(), "fixture")
            .await
            .expect("authenticate");
        Self {
            root: tempfile::tempdir().unwrap(),
            pg,
            owner,
            worker,
            repositories,
        }
    }
    fn docker(&self, mode: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = self.root.path().join("docker");
        let body = format!(
            r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> '{root}/calls'
printf '%s\n' "$DOCKER_CONFIG" >> '{root}/configs'
test "$HOME" = "$DOCKER_CONFIG"
test -z "${{AWS_SECRET_ACCESS_KEY:-}}"
case "$1 ${{2:-}}" in
 'ps -aq') [ '{mode}' != list_fail ] || exit 17; echo owned-container ;;
 'network ls') echo owned-network ;;
 'inspect --format'|'network inspect') [ '{mode}' != inspect_fail ] || exit 18; echo orphan-execution ;;
 'rm --force'|'network rm') [ '{mode}' != remove_fail ] || exit 19 ;;
 *) exit 20 ;;
esac
"#,
            root = self.root.path().display()
        );
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        script
    }
    fn config(&self, mode: &str) -> EvaluatorSupervisorConfig {
        EvaluatorSupervisorConfig {
            docker: self.docker(mode),
            workspace_root: self.root.path().to_path_buf(),
            environment: "fixture".into(),
            client_image: format!("fixture@sha256:{}", "a".repeat(64)),
            relay_image: format!("relay@sha256:{}", "b".repeat(64)),
            relay_control_network: "fixture-relay".into(),
            relay_upstream: "http://127.0.0.1:1".into(),
        }
    }
    async fn run(&self, mode: &str) -> Result<bool, String> {
        let supervisor = EvaluatorSupervisor::new(&self.repositories, self.config(mode)).unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            supervisor.run_once(&self.owner, &self.worker.id),
        )
        .await
        .expect("bounded supervisor fixture")
        .map_err(|e| e.to_string())
    }
    fn assert_private_configs_removed(&self) {
        for path in std::fs::read_to_string(self.root.path().join("configs"))
            .unwrap()
            .lines()
        {
            assert!(
                !std::path::Path::new(path).exists(),
                "Docker ambient-credential isolation directory must be removed"
            );
        }
    }
    async fn cleanup(self) {
        sqlx::query("DELETE FROM eval_workers WHERE owner_id=$1")
            .bind(self.owner.as_str())
            .execute(&self.pg)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(self.owner.as_str())
            .execute(&self.pg)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn empty_queue_reconciles_owned_orphans_and_removes_private_configuration() {
    let f = Fixture::new().await;
    assert!(!f.run("ok").await.unwrap());
    let calls = std::fs::read_to_string(f.root.path().join("calls")).unwrap();
    assert!(calls.contains(&format!("label=systemprompt.evaluator.owner={}", f.owner)));
    assert!(calls.contains("rm --force owned-container"));
    assert!(calls.contains("network rm owned-network"));
    assert!(!calls.lines().any(|line| line.starts_with("run ")));
    f.assert_private_configs_removed();
    assert!(
        !f.run("ok").await.unwrap(),
        "restart remains idle without creating reservations"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM eval_budget_accounts WHERE owner_id=$1")
            .bind(f.owner.as_str())
            .fetch_one(&f.pg)
            .await
            .unwrap(),
        0
    );
    f.cleanup().await;
}

#[tokio::test]
async fn reconciliation_errors_refuse_execution_and_still_clean_private_configuration() {
    let f = Fixture::new().await;
    for (mode, message) in [
        ("list_fail", "enumerate"),
        ("inspect_fail", "inspect"),
        ("remove_fail", "cleanup was not acknowledged"),
    ] {
        let error = f
            .run(mode)
            .await
            .expect_err("unverified cleanup must stop supervisor");
        assert!(error.contains(message), "{error}");
        f.assert_private_configs_removed();
    }
    f.cleanup().await;
}

#[tokio::test]
async fn foreign_or_disabled_worker_never_reaches_docker_or_reserves_budget() {
    let f = Fixture::new().await;
    let supervisor = EvaluatorSupervisor::new(&f.repositories, f.config("ok")).unwrap();
    assert!(
        supervisor
            .run_once(&UserId::new("foreign-owner"), &f.worker.id)
            .await
            .is_err()
    );
    assert!(
        supervisor
            .run_once(&f.owner, &EvalWorkerId::new("missing-worker"))
            .await
            .is_err()
    );
    sqlx::query("UPDATE eval_workers SET enabled=false WHERE id=$1")
        .bind(f.worker.id.as_str())
        .execute(&f.pg)
        .await
        .unwrap();
    assert!(supervisor.run_once(&f.owner, &f.worker.id).await.is_err());
    assert!(!f.root.path().join("calls").exists());
    f.cleanup().await;
}

#[tokio::test]
async fn invalid_supervisor_configuration_fails_before_external_or_database_side_effects() {
    let f = Fixture::new().await;
    for index in 0..7 {
        let mut config = f.config("ok");
        match index {
            0 => config.docker = PathBuf::from("docker"),
            1 => config.workspace_root = PathBuf::from("relative"),
            2 => config.environment = " ".into(),
            3 => config.client_image = "fixture:latest".into(),
            4 => config.relay_image = "relay:latest".into(),
            5 => config.relay_control_network = "host".into(),
            _ => config.relay_upstream = "file:///secret".into(),
        }
        assert!(EvaluatorSupervisor::new(&f.repositories, config).is_err());
    }
    assert!(!f.root.path().join("calls").exists());
    f.cleanup().await;
}
