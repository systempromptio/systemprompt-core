//! Real terminal persistence with owned workspace cleanup and PostgreSQL
//! leases. Fixture rows are already-running terminal inputs; no native target
//! is forged or enabled and no client process is launched.

use std::collections::BTreeMap;
use systemprompt_evaluation::experiments::execution::{
    ClientCapabilities, EvidenceArchive, ExecutionEvidence,
};
use systemprompt_evaluation::experiments::{ClientKind, VariantSpec};
use systemprompt_evaluation::repository::experiments::{
    EvaluationRepositories, ExecutionLease, ReservationAdmission, TerminalOutcome,
};
use systemprompt_identifiers::{
    AiRequestId, EvalBudgetId, EvalExecutionId, EvalExperimentId, EvalRevisionId, ModelId,
    ProviderId, UserId,
};
use systemprompt_scheduler::SchedulerError;
use systemprompt_scheduler::services::evaluator::adapters::NativeCompletion;
use systemprompt_scheduler::services::evaluator::supervisor::terminal::{
    CleanupResources, ExecutionTerminal, TerminalEvidence,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

struct Fixture {
    root: tempfile::TempDir,
    pg: sqlx::PgPool,
    repositories: EvaluationRepositories,
    owner: UserId,
    experiment: EvalExperimentId,
    budget: EvalBudgetId,
    lease: ExecutionLease,
    request: AiRequestId,
    variant: VariantSpec,
}
impl Fixture {
    async fn new() -> Self {
        let bootstrap = ensure_test_bootstrap();
        let pool = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("terminal fixture requires PostgreSQL");
        let pg = pool.write_pool_arc().unwrap().as_ref().clone();
        let owner = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row(&pool, &owner, &format!("{owner}@terminal.test"))
            .await
            .unwrap();
        let repositories = systemprompt_test_fixtures::fixture_evaluation_repositories(&pool)
            .expect("evaluation repositories");
        let budget = repositories
            .budgets
            .create_shared(&owner, "terminal-fixture", 10000)
            .await
            .unwrap();
        let credential = repositories
            .workers
            .create(&owner, "terminal-fixture", "terminal-fixture")
            .await
            .unwrap();
        let worker = repositories
            .workers
            .authenticate(credential.expose_token(), "terminal-fixture")
            .await
            .unwrap();
        let case = EvalRevisionId::generate();
        sqlx::query("INSERT INTO eval_resource_revisions(id,owner_id,resource_kind,resource_key,digest,content) VALUES($1,$2,'case','terminal-fixture',$3,'{}')")
            .bind(case.as_str()).bind(owner.as_str()).bind("a".repeat(64)).execute(&pg).await.unwrap();
        let variant = VariantSpec {
            client: ClientKind::ClaudeCode,
            client_version: "fixture-only".into(),
            model: ModelId::new("fixture-model"),
            provider: ProviderId::new("fixture-provider"),
            skill_bundle_digest: "a".repeat(64),
            configuration_digest: "b".repeat(64),
            worker_image_digest: "c".repeat(64),
        };
        let experiment = EvalExperimentId::generate();
        sqlx::query("INSERT INTO eval_experiments(id,owner_id,spec,spec_digest,budget_id,idempotency_key,status) VALUES($1,$2,$3,$4,$5,'terminal-fixture','running')")
            .bind(experiment.as_str()).bind(owner.as_str()).bind(serde_json::json!({"execution_mode":"fixture","variants":[variant.clone()]})).bind("d".repeat(64)).bind(budget.as_str()).execute(&pg).await.unwrap();
        let execution = EvalExecutionId::generate();
        sqlx::query("INSERT INTO eval_executions(id,experiment_id,variant_index,case_revision_id,repetition,status,lease_owner,lease_expires_at,deadline_at,last_heartbeat_at,fencing_token) VALUES($1,$2,0,$3,0,'running',$4,NOW()+INTERVAL '5 minutes',NOW()+INTERVAL '10 minutes',NOW(),1)")
            .bind(execution.as_str()).bind(experiment.as_str()).bind(case.as_str()).bind(worker.id.as_str()).execute(&pg).await.unwrap();
        let lease = ExecutionLease::builder(execution, worker.id)
            .fencing_token(1)
            .build()
            .unwrap();
        let access = repositories
            .capabilities
            .issue(&owner, &lease)
            .await
            .unwrap();
        let request = AiRequestId::generate();
        sqlx::query("INSERT INTO ai_requests(id,request_id,user_id,session_id,context_id,provider,model,status,actor_kind,actor_id) VALUES($1,$1,$2,$3,'00000000-0000-0000-0000-000000000001','fixture-provider','fixture-model','pending','job',$2)")
            .bind(request.as_str()).bind(owner.as_str()).bind(access.session_id.as_str()).execute(&pg).await.unwrap();
        let ReservationAdmission::Admitted(reservation) = repositories
            .budgets
            .reserve(&owner, &budget, request.as_str(), 400)
            .await
            .unwrap()
        else {
            panic!("new fixture reservation")
        };
        sqlx::query("INSERT INTO eval_request_reservations(request_id,execution_id,reservation_id,traffic_class) VALUES($1,$2,$3,'fixture')")
            .bind(request.as_str()).bind(lease.execution_id.as_str()).bind(reservation.as_str()).execute(&pg).await.unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("workspace")).unwrap();
        std::fs::write(
            root.path().join("workspace/pending-evidence"),
            b"owned fixture bytes",
        )
        .unwrap();
        Self {
            root,
            pg,
            repositories,
            owner,
            experiment,
            budget,
            lease,
            request,
            variant,
        }
    }
    fn terminal(&self) -> ExecutionTerminal {
        ExecutionTerminal::new(&self.repositories)
    }
    fn evidence(&self, cleaned: bool) -> ExecutionEvidence {
        ExecutionEvidence {
            execution_id: self.lease.execution_id.clone(),
            fencing_token: self.lease.fencing_token,
            capabilities: ClientCapabilities {
                client: self.variant.client,
                client_version: self.variant.client_version.clone(),
                adapter_version: "terminal-fixture-only".into(),
                image_digest: self.variant.worker_image_digest.clone(),
                supports_session_resume: false,
            },
            installed_bundle_digest: self.variant.skill_bundle_digest.clone(),
            candidate_bundle_digest: self.variant.skill_bundle_digest.clone(),
            workspace_digest: self.variant.configuration_digest.clone(),
            requests: vec![self.request.clone()],
            artifacts: vec![],
            exit_code: Some(0),
            elapsed_milliseconds: 1,
            cleanup_confirmed: cleaned,
        }
    }
    async fn budget(&self) -> (i64, i64) {
        let value = self
            .repositories
            .budgets
            .get(&self.owner, &self.budget)
            .await
            .unwrap();
        (value.reserved, value.settled)
    }
    async fn state(&self) -> serde_json::Value {
        sqlx::query_scalar("SELECT jsonb_build_object('status',x.status,'cleanup',(SELECT to_jsonb(c) FROM eval_execution_cleanup c WHERE c.execution_id=x.id),'evidence',(SELECT count(*) FROM eval_execution_evidence v WHERE v.execution_id=x.id),'measurements',(SELECT count(*) FROM eval_execution_measurements m WHERE m.execution_id=x.id),'suggestions',(SELECT count(*) FROM eval_suggestions s WHERE s.owner_id=e.owner_id)) FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=$1")
            .bind(self.lease.execution_id.as_str()).fetch_one(&self.pg).await.unwrap()
    }
    async fn cleanup_rows(self) {
        for statement in [
            "DELETE FROM eval_execution_artifacts WHERE execution_id IN (SELECT id FROM eval_executions WHERE experiment_id IN(SELECT id FROM eval_experiments WHERE owner_id=$1))",
            "DELETE FROM eval_execution_evidence WHERE execution_id IN (SELECT id FROM eval_executions WHERE experiment_id IN(SELECT id FROM eval_experiments WHERE owner_id=$1))",
            "DELETE FROM eval_execution_cleanup WHERE execution_id IN (SELECT id FROM eval_executions WHERE experiment_id IN(SELECT id FROM eval_experiments WHERE owner_id=$1))",
            "DELETE FROM eval_execution_capabilities WHERE execution_id IN (SELECT id FROM eval_executions WHERE experiment_id IN(SELECT id FROM eval_experiments WHERE owner_id=$1))",
            "DELETE FROM eval_request_reservations WHERE execution_id IN (SELECT id FROM eval_executions WHERE experiment_id IN(SELECT id FROM eval_experiments WHERE owner_id=$1))",
            "DELETE FROM eval_session_bindings WHERE owner_id=$1",
            "DELETE FROM ai_requests WHERE user_id=$1",
            "DELETE FROM eval_executions WHERE experiment_id IN(SELECT id FROM eval_experiments WHERE owner_id=$1)",
            "DELETE FROM eval_budget_reservations WHERE account_id IN(SELECT id FROM eval_budget_accounts WHERE owner_id=$1)",
            "DELETE FROM eval_experiments WHERE owner_id=$1",
            "DELETE FROM eval_budget_accounts WHERE owner_id=$1",
            "DELETE FROM eval_resource_revisions WHERE owner_id=$1",
            "DELETE FROM eval_workers WHERE owner_id=$1",
            "DELETE FROM user_sessions WHERE user_id=$1",
            "DELETE FROM users WHERE id=$1",
        ] {
            sqlx::query(statement)
                .bind(self.owner.as_str())
                .execute(&self.pg)
                .await
                .unwrap();
        }
    }
}
fn resources() -> CleanupResources<'static> {
    CleanupResources {
        container_id: Some("owned-fixture-client"),
        network_id: Some("owned-fixture-network"),
    }
}
fn archive() -> EvidenceArchive {
    EvidenceArchive {
        files: BTreeMap::new(),
    }
}

#[tokio::test]
async fn cancellation_cleanup_cannot_export_success_or_release_uncertain_spend() {
    let f = Fixture::new().await;
    let terminal = f.terminal();
    f.repositories
        .experiments
        .cancel(&f.owner, &f.experiment)
        .await
        .unwrap();
    let cleanup = terminal
        .cleanup(&f.owner, &f.lease, resources(), || {
            std::fs::remove_dir_all(f.root.path().join("workspace")).map_err(SchedulerError::from)
        })
        .await
        .unwrap();
    assert!(cleanup.verified());
    assert!(!f.root.path().join("workspace").exists());
    assert!(
        terminal
            .persist(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &f.evidence(true),
                    archive: &archive(),
                    cleanup: &cleanup
                },
                NativeCompletion::Completed
            )
            .await
            .is_err()
    );
    let state = f.state().await;
    assert_eq!(state["status"], "cancelled");
    assert_eq!(state["cleanup"]["status"], "verified");
    assert_eq!(state["evidence"], 0);
    assert_eq!(state["measurements"], 0);
    assert_eq!(state["suggestions"], 0);
    assert_eq!(f.budget().await, (400, 0));
    f.repositories
        .lifecycle
        .reconcile_restart(&f.owner)
        .await
        .unwrap();
    assert_eq!(f.budget().await, (0, 400));
    f.repositories
        .lifecycle
        .reconcile_restart(&f.owner)
        .await
        .unwrap();
    assert_eq!(f.budget().await, (0, 400));
    f.cleanup_rows().await;
}
#[tokio::test]
async fn stale_fence_cannot_overwrite_cleanup_or_persist_terminal_evidence() {
    let f = Fixture::new().await;
    let terminal = f.terminal();
    let cleanup = terminal
        .cleanup(&f.owner, &f.lease, resources(), || Ok(()))
        .await
        .unwrap();
    let before = f.state().await;
    sqlx::query("UPDATE eval_executions SET fencing_token=fencing_token+1 WHERE id=$1")
        .bind(f.lease.execution_id.as_str())
        .execute(&f.pg)
        .await
        .unwrap();
    assert!(
        terminal
            .cleanup(&f.owner, &f.lease, resources(), || Err(
                SchedulerError::config_error("stale cleanup must not replace current state")
            ))
            .await
            .is_err()
    );
    assert!(
        terminal
            .persist(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &f.evidence(true),
                    archive: &archive(),
                    cleanup: &cleanup
                },
                NativeCompletion::Completed
            )
            .await
            .is_err()
    );
    assert_eq!(f.state().await, before);
    assert_eq!(f.budget().await, (400, 0));
    f.cleanup_rows().await;
}
#[tokio::test]
async fn failed_cleanup_witness_prevents_success_and_preserves_restart_bound() {
    let f = Fixture::new().await;
    let terminal = f.terminal();
    let cleanup = terminal
        .cleanup(&f.owner, &f.lease, resources(), || {
            std::fs::remove_dir_all(f.root.path().join("workspace"))?;
            Err(SchedulerError::config_error(
                "network removal was not acknowledged",
            ))
        })
        .await
        .unwrap();
    assert!(!cleanup.verified());
    assert!(!f.root.path().join("workspace").exists());
    assert!(
        terminal
            .persist(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &f.evidence(true),
                    archive: &archive(),
                    cleanup: &cleanup
                },
                NativeCompletion::Completed
            )
            .await
            .is_err(),
        "a caller cannot upgrade failed cleanup to verified evidence"
    );
    assert_eq!(
        terminal
            .persist(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &f.evidence(false),
                    archive: &archive(),
                    cleanup: &cleanup
                },
                NativeCompletion::Completed
            )
            .await
            .unwrap(),
        TerminalOutcome::Error
    );
    let state = f.state().await;
    assert_eq!(state["status"], "error");
    assert_eq!(state["cleanup"]["status"], "failed");
    assert!(
        state["cleanup"]["last_error"]
            .as_str()
            .unwrap()
            .contains("not acknowledged")
    );
    assert_eq!(state["evidence"], 1);
    assert_eq!(state["measurements"], 0);
    assert_eq!(state["suggestions"], 0);
    assert_eq!(f.budget().await, (400, 0));
    f.repositories
        .lifecycle
        .reconcile_restart(&f.owner)
        .await
        .unwrap();
    assert_eq!(f.budget().await, (0, 400));
    f.cleanup_rows().await;
}

#[tokio::test]
async fn native_start_failure_is_retained_without_waiting_for_lease_expiry() {
    let f = Fixture::new().await;
    let terminal = ExecutionTerminal::new(&f.repositories);
    let before = f.budget().await;
    let cleanup = terminal
        .cleanup(&f.owner, &f.lease, resources(), || {
            std::fs::remove_dir_all(f.root.path().join("workspace"))?;
            Ok(())
        })
        .await
        .unwrap();
    let reason = "Native client blocked: Pinned image configuration could not be established";
    assert_eq!(
        terminal
            .block(&f.owner, &f.lease, &cleanup, reason)
            .await
            .unwrap(),
        TerminalOutcome::Blocked
    );
    let state = f.state().await;
    assert_eq!(state["status"], "blocked");
    assert_eq!(state["evidence"], 0);
    assert_eq!(state["cleanup"]["status"], "verified");
    let retained: String =
        sqlx::query_scalar("SELECT result->>'summary' FROM eval_executions WHERE id=$1")
            .bind(f.lease.execution_id.as_str())
            .fetch_one(&f.pg)
            .await
            .unwrap();
    assert_eq!(retained, reason);
    assert_eq!(f.budget().await, before);
    f.cleanup_rows().await;
}

#[tokio::test]
async fn judge_start_failure_keeps_native_evidence_and_prior_spend() {
    let f = Fixture::new().await;
    let terminal = ExecutionTerminal::new(&f.repositories);
    let before = f.budget().await;
    let cleanup = terminal
        .cleanup(&f.owner, &f.lease, resources(), || Ok(()))
        .await
        .unwrap();
    assert_eq!(
        terminal
            .persist_blocked(
                &f.owner,
                &f.lease,
                TerminalEvidence {
                    evidence: &f.evidence(true),
                    archive: &archive(),
                    cleanup: &cleanup
                },
                "Native judge blocked: Pinned executable bytes do not match native admission"
            )
            .await
            .unwrap(),
        TerminalOutcome::Blocked
    );
    let state = f.state().await;
    assert_eq!(state["status"], "blocked");
    assert_eq!(state["evidence"], 1);
    assert_eq!(state["measurements"], 0);
    assert_eq!(f.budget().await, before);
    let mut stale = f.lease.clone();
    stale.fencing_token += 1;
    assert!(
        terminal
            .block(&f.owner, &stale, &cleanup, "replacement failure")
            .await
            .is_err()
    );
    f.cleanup_rows().await;
}

#[derive(Debug)]
struct RejectNativePin(&'static str);
impl systemprompt_scheduler::services::evaluator::container::ClientVerifier for RejectNativePin {
    fn verify(
        &self,
        _launch: &systemprompt_scheduler::services::evaluator::container::ContainerLaunch,
        _client: &systemprompt_scheduler::services::evaluator::client::NativeClient,
    ) -> systemprompt_scheduler::SchedulerResult<()> {
        Err(SchedulerError::config_error(self.0))
    }
}

#[tokio::test]
async fn supervisor_start_boundary_blocks_missing_image_wrong_config_and_executable() {
    use systemprompt_scheduler::services::evaluator::client::{ClientPurpose, NativeClient};
    use systemprompt_scheduler::services::evaluator::container::ContainerLaunch;
    use systemprompt_scheduler::services::evaluator::supervisor::terminal::NativeStart;
    for reason in [
        "Pinned image configuration could not be established",
        "Image manifest resolved to a different retained config identity",
        "Pinned executable bytes do not match native admission",
    ] {
        let f = Fixture::new().await;
        let before = f.budget().await;
        let launch = ContainerLaunch::builder(
            f.root.path().join("must-not-be-executed"),
            f.root.path().to_owned(),
        )
        .image(format!("sha256:{}", "c".repeat(64)))
        .network("fixture-network".to_owned())
        .name("eval-fixture-client".to_owned())
        .ownership(f.owner.as_str(), f.lease.execution_id.as_str())
        .lease(&f.lease)
        .verifier(std::sync::Arc::new(RejectNativePin(reason)))
        .build()
        .unwrap();
        let client = NativeClient::builder(
            systemprompt_evaluation::experiments::ClientKind::ClaudeCode,
            ModelId::new("fixture-model"),
        )
        .build()
        .unwrap();
        let terminal = ExecutionTerminal::new(&f.repositories);
        let cleanup_called = std::cell::Cell::new(false);
        let started = terminal
            .start_client(
                &f.owner,
                &f.lease,
                NativeStart {
                    launch: &launch,
                    client: &client,
                    purpose: ClientPurpose::Execution,
                    prompt: "fixture",
                    readiness: None,
                },
                || {
                    cleanup_called.set(true);
                    std::fs::remove_dir_all(f.root.path().join("workspace"))?;
                    Ok(())
                },
            )
            .await
            .unwrap();
        assert!(started.is_none());
        assert!(cleanup_called.get());
        assert!(!f.root.path().join("client-events.jsonl").exists());
        assert_eq!(f.state().await["status"], "blocked");
        let retained: String =
            sqlx::query_scalar("SELECT result->>'summary' FROM eval_executions WHERE id=$1")
                .bind(f.lease.execution_id.as_str())
                .fetch_one(&f.pg)
                .await
                .unwrap();
        assert!(retained.contains(reason));
        assert_eq!(f.budget().await, before);
        f.cleanup_rows().await;
    }
}

#[tokio::test]
async fn stale_cleanup_fence_never_executes_resource_removal() {
    let f = Fixture::new().await;
    let terminal = ExecutionTerminal::new(&f.repositories);
    let mut stale = f.lease.clone();
    stale.fencing_token += 1;
    let called = std::cell::Cell::new(false);
    assert!(
        terminal
            .cleanup(&f.owner, &stale, resources(), || {
                called.set(true);
                Ok(())
            })
            .await
            .is_err()
    );
    assert!(!called.get());
    assert!(f.root.path().join("workspace").exists());
    f.cleanup_rows().await;
}

#[path = "evaluator_terminal_acceptance.rs"]
mod acceptance;

#[path = "evaluator_supervisor_lease_recovery.rs"]
mod supervisor_lease_recovery;
