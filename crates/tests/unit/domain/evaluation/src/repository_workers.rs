//! DB-backed tests for `WorkerRepository` and the shared experiment harness
//! the other `repository_*` experiment suites build on.
//!
//! Every harness owns a freshly generated user id, so seeded experiments,
//! workers, revisions and sessions are namespaced per test and removed by
//! `Harness::cleanup`.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_evaluation::experiments::execution::FrozenWorkspace;
use systemprompt_evaluation::experiments::records::ExecutionRecord;
use systemprompt_evaluation::experiments::resources::{
    CaseContent, Partition, ResourceContent, RubricContent, WeightedDimension,
};
use systemprompt_evaluation::experiments::{
    ClientKind, ExecutionMode, ExperimentSpec, Objective, VariantSpec,
};
use systemprompt_evaluation::repository::experiments::{
    EvidenceRepository, ExecutionLease, ExperimentRepository, RevisionRepository, WorkerRecord,
    WorkerRepository,
};
use systemprompt_identifiers::{
    EvalExecutionId, EvalExperimentId, EvalRevisionId, EvalWorkerId, ModelId, ProviderId, UserId,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

pub const MODEL: &str = "claude-sonnet-5";
pub const PROVIDER: &str = "anthropic";

pub struct Harness {
    pub pool: systemprompt_database::DbPool,
    pub pg: PgPool,
    pub owner: UserId,
    pub environment: String,
    pub worker: WorkerRecord,
    pub worker_token: String,
    pub experiment: EvalExperimentId,
    pub case_revision: EvalRevisionId,
    pub rubric_revision: EvalRevisionId,
    pub bundle_digest: String,
    pub configuration_digest: String,
}

pub fn rubric_content() -> RubricContent {
    RubricContent {
        dimensions: vec![WeightedDimension {
            name: "grounding".to_owned(),
            description: "Claims cite evidence".to_owned(),
            weight: 2,
        }],
        pass_threshold_milli: 3000,
        hard_gates: vec!["approval".to_owned()],
    }
}

pub fn case_content() -> CaseContent {
    CaseContent {
        prompt: "Write a specification".to_owned(),
        expected_behavior: vec!["Cites sources".to_owned()],
        fixtures: BTreeMap::from([("README.md".to_owned(), "fixture".to_owned())]),
        partition: Partition::Development,
    }
}

fn workspace(marker: &str) -> FrozenWorkspace {
    FrozenWorkspace {
        files: BTreeMap::from([("SKILL.md".to_owned(), marker.to_owned())]),
    }
}

impl Harness {
    pub async fn start() -> Option<Self> {
        Self::start_with_repetitions(1).await
    }

    pub async fn start_with_repetitions(repetitions: u32) -> Option<Self> {
        let url = fixture_database_url().ok()?;
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let pg = (*pool.write_pool_arc().expect("write pool")).clone();
        let owner = unique_user_id("eval-owner");
        seed_user_row(&pool, &owner, &format!("{}@example.test", owner.as_str()))
            .await
            .expect("seed owner");

        let evidence = EvidenceRepository::new(pg.clone());
        let bundle_digest = evidence
            .save_workspace(&owner, &workspace("bundle"))
            .await
            .expect("bundle workspace");
        let configuration_digest = evidence
            .save_workspace(&owner, &workspace("configuration"))
            .await
            .expect("configuration workspace");

        let revisions = RevisionRepository::new(pg.clone());
        let rubric_revision = revisions
            .create(&owner, "rubric", &ResourceContent::Rubric(rubric_content()))
            .await
            .expect("rubric revision");
        let case_revision = revisions
            .create(&owner, "case", &ResourceContent::Case(case_content()))
            .await
            .expect("case revision");

        let spec = ExperimentSpec {
            schema_version: 1,
            name: "harness".to_owned(),
            cases: vec![case_revision.clone()],
            rubric: rubric_revision.clone(),
            variants: vec![VariantSpec {
                client: ClientKind::ClaudeCode,
                client_version: "1.0.0".to_owned(),
                model: ModelId::new(MODEL),
                provider: ProviderId::new(PROVIDER),
                skill_bundle_digest: bundle_digest.clone(),
                configuration_digest: configuration_digest.clone(),
                worker_image_digest: "c".repeat(64),
            }],
            repetitions,
            budget_microdollars: 5_000_000,
            execution_mode: ExecutionMode::Fixture,
            objective: Objective::Quality,
        };
        let experiment = ExperimentRepository::new(pg.clone())
            .create(&owner, &format!("key-{}", Uuid::new_v4()), &spec)
            .await
            .expect("create experiment");

        let environment = format!("env-{}", Uuid::new_v4());
        let workers = WorkerRepository::new(pg.clone());
        let credential = workers
            .create(&owner, &environment, "harness-worker")
            .await
            .expect("create worker");
        let worker_token = credential.expose_token().to_owned();
        let worker = workers
            .authenticate(&worker_token, &environment)
            .await
            .expect("authenticate worker");

        Some(Self {
            pool,
            pg,
            owner,
            environment,
            worker,
            worker_token,
            experiment,
            case_revision,
            rubric_revision,
            bundle_digest,
            configuration_digest,
        })
    }

    pub fn experiments(&self) -> ExperimentRepository {
        ExperimentRepository::new(self.pg.clone())
    }

    pub fn workers(&self) -> WorkerRepository {
        WorkerRepository::new(self.pg.clone())
    }

    pub async fn claim(&self) -> ExecutionRecord {
        self.experiments()
            .claim(&self.owner, &self.worker.id)
            .await
            .expect("claim")
            .expect("an execution is queued")
    }

    pub async fn claimed_lease(&self) -> (ExecutionRecord, ExecutionLease) {
        let execution = self.claim().await;
        let lease = ExecutionLease::builder(execution.id.clone(), self.worker.id.clone())
            .fencing_token(execution.fencing_token)
            .build()
            .expect("lease");
        (execution, lease)
    }

    pub async fn set_lease_expiry(&self, execution: &EvalExecutionId, seconds: f64) {
        sqlx::query(
            "UPDATE eval_executions SET lease_expires_at = NOW() + make_interval(secs => $2) \
             WHERE id = $1",
        )
        .bind(execution.as_str())
        .bind(seconds)
        .execute(&self.pg)
        .await
        .expect("set lease expiry");
    }

    pub async fn lease_expiry(&self, execution: &EvalExecutionId) -> DateTime<Utc> {
        sqlx::query_scalar::<_, DateTime<Utc>>(
            "SELECT lease_expires_at FROM eval_executions WHERE id = $1",
        )
        .bind(execution.as_str())
        .fetch_one(&self.pg)
        .await
        .expect("read lease expiry")
    }

    pub async fn execution_status(&self, execution: &EvalExecutionId) -> String {
        sqlx::query_scalar::<_, String>("SELECT status FROM eval_executions WHERE id = $1")
            .bind(execution.as_str())
            .fetch_one(&self.pg)
            .await
            .expect("read execution status")
    }

    pub async fn experiment_status(&self) -> String {
        sqlx::query_scalar::<_, String>("SELECT status FROM eval_experiments WHERE id = $1")
            .bind(self.experiment.as_str())
            .fetch_one(&self.pg)
            .await
            .expect("read experiment status")
    }

    pub async fn budget(&self) -> (i64, i64) {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT b.reserved, b.settled FROM eval_budget_accounts b JOIN eval_experiments e ON \
             e.budget_id = b.id WHERE e.id = $1",
        )
        .bind(self.experiment.as_str())
        .fetch_one(&self.pg)
        .await
        .expect("read budget")
    }

    pub async fn seed_pending_request(
        &self,
        session: &str,
    ) -> systemprompt_identifiers::AiRequestId {
        let id = format!("eval-req-{}", Uuid::new_v4());
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, session_id, context_id, provider, \
             model, status, actor_kind, actor_id) VALUES ($1, $1, $2, $3, \
             '00000000-0000-0000-0000-00000000c0de', $4, $5, 'pending', 'job', $2)",
        )
        .bind(&id)
        .bind(self.owner.as_str())
        .bind(session)
        .bind(PROVIDER)
        .bind(MODEL)
        .execute(&self.pg)
        .await
        .expect("seed pending request");
        systemprompt_identifiers::AiRequestId::new(id)
    }

    pub async fn complete_request(
        &self,
        request: &systemprompt_identifiers::AiRequestId,
        cost: i64,
    ) {
        sqlx::query(
            "UPDATE ai_requests SET status = 'completed', completed_at = NOW(), tokens_used = 100, \
             cost_microdollars = $2 WHERE id = $1",
        )
        .bind(request.as_str())
        .bind(cost)
        .execute(&self.pg)
        .await
        .expect("complete request");
    }

    pub async fn cleanup(&self) {
        for statement in [
            "DELETE FROM eval_execution_capabilities WHERE execution_id IN (SELECT x.id FROM \
             eval_executions x JOIN eval_experiments e ON e.id = x.experiment_id WHERE e.owner_id \
             = $1)",
            "DELETE FROM eval_execution_events WHERE execution_id IN (SELECT x.id FROM \
             eval_executions x JOIN eval_experiments e ON e.id = x.experiment_id WHERE e.owner_id \
             = $1)",
            "DELETE FROM eval_execution_artifacts WHERE execution_id IN (SELECT x.id FROM \
             eval_executions x JOIN eval_experiments e ON e.id = x.experiment_id WHERE e.owner_id \
             = $1)",
            "DELETE FROM eval_execution_evidence WHERE execution_id IN (SELECT x.id FROM \
             eval_executions x JOIN eval_experiments e ON e.id = x.experiment_id WHERE e.owner_id \
             = $1)",
            "DELETE FROM eval_request_reservations WHERE execution_id IN (SELECT x.id FROM \
             eval_executions x JOIN eval_experiments e ON e.id = x.experiment_id WHERE e.owner_id \
             = $1)",
            "DELETE FROM eval_session_bindings WHERE owner_id = $1",
            "DELETE FROM ai_requests WHERE user_id = $1",
            "DELETE FROM eval_executions WHERE experiment_id IN (SELECT id FROM eval_experiments \
             WHERE owner_id = $1)",
            "DELETE FROM eval_budget_reservations WHERE account_id IN (SELECT id FROM \
             eval_budget_accounts WHERE owner_id = $1)",
            "DELETE FROM eval_experiments WHERE owner_id = $1",
            "DELETE FROM eval_budget_accounts WHERE owner_id = $1",
            "DELETE FROM eval_resource_revisions WHERE owner_id = $1",
            "DELETE FROM eval_frozen_workspaces WHERE owner_id = $1",
            "DELETE FROM eval_workers WHERE owner_id = $1",
            "DELETE FROM user_sessions WHERE user_id = $1",
            "DELETE FROM users WHERE id = $1",
        ] {
            sqlx::query(statement)
                .bind(self.owner.as_str())
                .execute(&self.pg)
                .await
                .expect("cleanup");
        }
    }
}

#[tokio::test]
async fn worker_credential_authenticates_only_in_its_environment() {
    let Some(harness) = Harness::start().await else {
        return;
    };

    let record = harness
        .workers()
        .authenticate(&harness.worker_token, &harness.environment)
        .await
        .expect("authenticate");
    assert_eq!(record.owner_id, harness.owner);
    assert_eq!(record.environment, harness.environment);
    assert_eq!(record.name, "harness-worker");
    assert!(record.enabled);
    assert!(record.expires_at > Utc::now());

    let foreign = harness
        .workers()
        .authenticate(&harness.worker_token, "another-environment")
        .await;
    assert!(foreign.is_err());

    harness.cleanup().await;
}

#[tokio::test]
async fn revoked_worker_credential_stops_authenticating() {
    let Some(harness) = Harness::start().await else {
        return;
    };

    harness
        .workers()
        .revoke(&harness.owner, &harness.worker.id)
        .await
        .expect("revoke");
    assert!(
        harness
            .workers()
            .authenticate(&harness.worker_token, &harness.environment)
            .await
            .is_err()
    );

    let repeat = harness
        .workers()
        .revoke(&harness.owner, &harness.worker.id)
        .await;
    assert!(repeat.is_ok(), "a second revoke still matches the row");

    let foreign = harness
        .workers()
        .revoke(&UserId::new("someone-else"), &harness.worker.id)
        .await;
    assert!(foreign.is_err(), "revocation is owner scoped");

    harness.cleanup().await;
}

#[tokio::test]
async fn worker_creation_rejects_blank_or_oversized_identity() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let workers = harness.workers();

    for (environment, name) in [
        ("   ".to_owned(), "worker".to_owned()),
        ("env".to_owned(), "   ".to_owned()),
        ("e".repeat(513), "worker".to_owned()),
        ("env".to_owned(), "n".repeat(129)),
    ] {
        assert!(
            workers
                .create(&harness.owner, &environment, &name)
                .await
                .is_err(),
            "environment {} / name {} must be rejected",
            environment.len(),
            name.len()
        );
    }

    harness.cleanup().await;
}

#[tokio::test]
async fn worker_authentication_rejects_malformed_tokens() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let workers = harness.workers();

    for token in [
        "not-a-worker-token".to_owned(),
        format!("speval_{}", "x".repeat(200)),
        format!("speval_{}", Uuid::new_v4()),
    ] {
        assert!(
            workers
                .authenticate(&token, &harness.environment)
                .await
                .is_err(),
            "token {token} must be rejected"
        );
    }

    harness.cleanup().await;
}

#[test]
fn worker_record_builder_requires_environment_and_name() {
    let expiry = Utc::now() + chrono::Duration::days(1);
    let record = WorkerRecord::builder(EvalWorkerId::generate(), UserId::new("owner"))
        .environment("staging".to_owned())
        .name("runner".to_owned())
        .enabled(false)
        .expires_at(expiry)
        .build()
        .expect("record");
    assert_eq!(record.environment, "staging");
    assert_eq!(record.name, "runner");
    assert!(!record.enabled);
    assert_eq!(record.expires_at, expiry);

    assert!(
        WorkerRecord::builder(EvalWorkerId::generate(), UserId::new("owner"))
            .name("runner".to_owned())
            .build()
            .is_err()
    );
    assert!(
        WorkerRecord::builder(EvalWorkerId::generate(), UserId::new("owner"))
            .environment("staging".to_owned())
            .build()
            .is_err()
    );
}
