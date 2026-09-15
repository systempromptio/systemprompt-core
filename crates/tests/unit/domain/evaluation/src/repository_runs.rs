//! DB-backed tests for the experiment repository: idempotent creation of the
//! frozen execution matrix, owner-scoped reads, cancellation and the durable
//! worker claim. Each test uses a fresh UUID owner so the owner-scoped
//! advisory lock and the queue assertions never observe another test's rows.

use sqlx::PgPool;
use std::collections::BTreeMap;
use systemprompt_evaluation::EvaluationError;
use systemprompt_evaluation::experiments::records::{ExecutionStatus, ExperimentStatus};
use systemprompt_evaluation::experiments::resources::{
    CaseContent, Partition, ResourceContent, RubricContent, WeightedDimension,
};
use systemprompt_evaluation::experiments::{
    ClientKind, ExecutionMode, ExperimentSpec, FrozenCostEnvelope, FrozenSettings, Objective,
    VariantSpec, content_digest,
};
use systemprompt_evaluation::repository::experiments::{ExperimentRepository, RevisionRepository};
use systemprompt_identifiers::{
    EvalBudgetId, EvalExperimentId, EvalRevisionId, EvalWorkerId, ManagedResourceId, ModelId,
    ProviderId, ResourceRevisionId, UserId,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn runs_pool() -> Option<PgPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let write = pool.write_pool_arc().expect("write pool");
    Some(write.as_ref().clone())
}

async fn new_owner(pool: &PgPool) -> UserId {
    let owner = UserId::new(format!("eval-runs-{}", Uuid::new_v4()));
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(owner.as_str())
        .bind(format!("{}@eval.invalid", owner.as_str()))
        .execute(pool)
        .await
        .expect("seed owner");
    owner
}

fn case_content(prompt: &str) -> ResourceContent {
    ResourceContent::Case(CaseContent {
        prompt: prompt.to_owned(),
        expected_behavior: vec!["Cites its sources".to_owned()],
        fixtures: BTreeMap::new(),
        partition: Partition::Development,
        assertions: vec!["response_present".to_owned()],
    })
}

fn rubric_content() -> ResourceContent {
    ResourceContent::Rubric(RubricContent {
        dimensions: vec![WeightedDimension {
            name: "grounding".to_owned(),
            description: "Claims are supported".to_owned(),
            weight: 1,
        }],
        pass_threshold_milli: 3000,
        hard_gates: Vec::new(),
    })
}

fn variant(bundle_digest: &str) -> VariantSpec {
    VariantSpec {
        client: ClientKind::ClaudeCode,
        client_version: "1.0.0".to_owned(),
        model: ModelId::new("claude-sonnet-5"),
        provider: ProviderId::new("anthropic"),
        skill_bundle_digest: bundle_digest.to_owned(),
        configuration_digest: "b".repeat(64),
        worker_image_digest: "c".repeat(64),
    }
}

struct Fixture {
    experiments: ExperimentRepository,
    owner: UserId,
    case: EvalRevisionId,
    rubric: EvalRevisionId,
    dataset: EvalRevisionId,
    dataset_digest: String,
    rubric_digest: String,
    budget: EvalBudgetId,
    resource: ManagedResourceId,
    baseline: ResourceRevisionId,
}

impl Fixture {
    fn spec(
        &self,
        cases: Vec<EvalRevisionId>,
        rubric: EvalRevisionId,
        repetitions: u32,
    ) -> ExperimentSpec {
        let executions =
            i64::try_from(cases.len()).expect("case count") * 2 * i64::from(repetitions);
        ExperimentSpec {
            schema_version: 1,
            name: "matrix".to_owned(),
            cases,
            rubric,
            dataset: Some(self.dataset.clone()),
            variants: vec![variant(&"a".repeat(64)), variant(&"d".repeat(64))],
            repetitions,
            budget_microdollars: executions,
            execution_mode: ExecutionMode::Fixture,
            objective: Objective::Quality,
            frozen: Some(FrozenSettings {
                provider_prices_digest: "e".repeat(64),
                tool_configuration_digest: "f".repeat(64),
                fixture_clock: "2026-09-12T08:00:00Z".to_owned(),
                fixture_timezone: "UTC".to_owned(),
                permissions_digest: "1".repeat(64),
                dataset_digest: self.dataset_digest.clone(),
                rubric_digest: self.rubric_digest.clone(),
                cost_envelope: FrozenCostEnvelope {
                    maximum_attempts_per_execution: 1,
                    generation_microdollars_per_attempt: 1,
                    judging_microdollars_per_attempt: 0,
                    tool_microdollars_per_attempt: 0,
                    suggestion_calls: 0,
                    suggestion_microdollars_per_call: 0,
                    auxiliary_calls: 0,
                    auxiliary_microdollars_per_call: 0,
                },
            }),
            claim_independent_improvement: false,
        }
    }
}

async fn fixture(pool: &PgPool) -> Fixture {
    let owner = new_owner(pool).await;
    let revisions = RevisionRepository::new(pool.clone());
    let case = revisions
        .create(&owner, "case-key", &case_content("Write a specification"))
        .await
        .expect("case revision");
    let rubric = revisions
        .create(&owner, "rubric-key", &rubric_content())
        .await
        .expect("rubric revision");
    let dataset_content = ResourceContent::Dataset(vec![case.clone()]);
    let dataset = revisions
        .create(&owner, "dataset-key", &dataset_content)
        .await
        .expect("dataset revision");
    for (revision, digest) in [
        ("base", "a".repeat(64)),
        ("configuration", "b".repeat(64)),
        ("candidate", "d".repeat(64)),
    ] {
        let manifest = serde_json::json!({"projection": revision});
        sqlx::query!("INSERT INTO eval_managed_workspace_projections(owner_id,digest,managed_revision_id,manifest,verified_file_count,verified_byte_count) VALUES($1,$2,$3,$4,0,0)", owner.as_str(), &digest, revision, manifest).execute(pool).await.expect("managed projection");
    }
    let budget = crate::seams::budgets(&pool)
        .create_shared(&owner, &format!("budget-{}", Uuid::new_v4()), 5_000_000)
        .await
        .expect("budget");
    let (resource, baseline) = systemprompt_test_fixtures::seed_managed_baseline(
        &crate::seams::db(pool),
        &owner,
        &format!("baseline-{}", Uuid::new_v4()),
    )
    .await
    .expect("managed baseline");
    Fixture {
        experiments: crate::seams::experiments(
            &pool,
            crate::fixture_admission::fixture_admission(),
        ),
        owner,
        case,
        dataset,
        dataset_digest: content_digest(&dataset_content).expect("dataset digest"),
        rubric_digest: content_digest(&rubric_content()).expect("rubric digest"),
        rubric,
        budget,
        resource,
        baseline,
    }
}

#[tokio::test]
async fn create_freezes_the_full_execution_matrix() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 2);

    let id = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");

    let detail = f.experiments.get(&f.owner, &id).await.expect("get");
    assert_eq!(detail.experiment.id, id);
    assert_eq!(detail.experiment.status, ExperimentStatus::Queued);
    assert_eq!(detail.experiment.accounting.cap, 5_000_000);
    assert_eq!(detail.experiment.accounting.reserved, 0);
    assert_eq!(
        detail.executions.len(),
        4,
        "2 variants x 1 case x 2 repetitions"
    );
    let coordinates: Vec<(i32, i32)> = detail
        .executions
        .iter()
        .map(|execution| (execution.variant_index, execution.repetition))
        .collect();
    assert_eq!(
        coordinates,
        vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        "executions are ordered by variant, case and repetition"
    );
    assert!(
        detail
            .executions
            .iter()
            .all(|execution| execution.status == ExecutionStatus::Queued
                && execution.fencing_token == 0
                && execution.result.is_none())
    );
}

#[tokio::test]
async fn create_is_idempotent_per_key_and_conflicts_on_a_changed_spec() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let first_spec = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);

    let id = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &first_spec)
        .await
        .expect("create");
    let repeated = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &first_spec)
        .await
        .expect("repeat");
    assert_eq!(
        repeated, id,
        "a replayed key must not queue a second matrix"
    );
    assert_eq!(
        f.experiments
            .get(&f.owner, &id)
            .await
            .expect("get")
            .executions
            .len(),
        2,
        "the replay must not duplicate executions"
    );

    let mut changed = first_spec;
    changed.name = "different".to_owned();
    assert!(matches!(
        f.experiments
            .create_with_budget(&f.owner, "key-1", &f.budget, &changed)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
}

#[tokio::test]
async fn create_rejects_invalid_keys_and_mistyped_resource_references() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let valid = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);

    for key in ["", "   "] {
        assert!(
            matches!(
                f.experiments
                    .create_with_budget(&f.owner, key, &f.budget, &valid)
                    .await,
                Err(EvaluationError::InvalidSpec(_))
            ),
            "key {key:?} must be rejected"
        );
    }
    let long_key = "k".repeat(256);
    assert!(matches!(
        f.experiments
            .create_with_budget(&f.owner, &long_key, &f.budget, &valid)
            .await,
        Err(EvaluationError::InvalidSpec(_))
    ));

    let mut empty_matrix = valid.clone();
    empty_matrix.variants.clear();
    assert!(
        matches!(
            f.experiments
                .create_with_budget(&f.owner, "key-2", &f.budget, &empty_matrix)
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "the spec is validated before anything is persisted"
    );

    let swapped = f.spec(vec![f.rubric.clone()], f.case.clone(), 1);
    assert!(
        matches!(
            f.experiments
                .create_with_budget(&f.owner, "key-3", &f.budget, &swapped)
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a case revision cannot stand in for the rubric"
    );

    let case_is_rubric = f.spec(vec![f.rubric.clone()], f.rubric.clone(), 1);
    assert!(
        matches!(
            f.experiments
                .create_with_budget(&f.owner, "key-4", &f.budget, &case_is_rubric)
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a rubric revision cannot stand in for a case"
    );

    let foreign = f.spec(vec![EvalRevisionId::generate()], f.rubric.clone(), 1);
    assert!(matches!(
        f.experiments
            .create_with_budget(&f.owner, "key-5", &f.budget, &foreign)
            .await,
        Err(EvaluationError::InvalidSpec(_))
    ));
}

#[tokio::test]
async fn reads_are_owner_scoped_and_listed_newest_first() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);

    let first = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");
    sqlx::query("UPDATE eval_experiments SET created_at = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(first.as_str())
        .execute(&pool)
        .await
        .expect("age the first experiment");
    let second = f
        .experiments
        .create_with_budget(&f.owner, "key-2", &f.budget, &matrix)
        .await
        .expect("create");

    let listed = f.experiments.list(&f.owner).await.expect("list");
    let ids: Vec<&EvalExperimentId> = listed.iter().map(|record| &record.id).collect();
    assert_eq!(ids, vec![&second, &first], "newest first");
    assert!(listed.iter().all(|record| record.owner_id == f.owner));

    let stranger = new_owner(&pool).await;
    assert!(
        f.experiments
            .list(&stranger)
            .await
            .expect("list")
            .is_empty()
    );
    assert!(matches!(
        f.experiments.get(&stranger, &first).await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(matches!(
        f.experiments
            .get(&f.owner, &EvalExperimentId::generate())
            .await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
}

#[tokio::test]
async fn cancellation_leaves_the_budget_open_and_drains_only_its_queue() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 2);
    let id = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");

    assert!(
        matches!(
            f.experiments.cancel(&new_owner(&pool).await, &id).await,
            Err(EvaluationError::ExperimentConflict(_))
        ),
        "cancellation is owner-scoped"
    );

    f.experiments.cancel(&f.owner, &id).await.expect("cancel");
    let detail = f.experiments.get(&f.owner, &id).await.expect("get");
    assert_eq!(detail.experiment.status, ExperimentStatus::Cancelled);
    assert!(!detail.experiment.accounting.frozen);
    assert!(
        detail
            .executions
            .iter()
            .all(|execution| execution.status == ExecutionStatus::Cancelled
                && execution.finished_at.is_some())
    );

    assert!(
        f.experiments
            .claim(&f.owner, &EvalWorkerId::new("worker-1"))
            .await
            .expect("claim")
            .is_none(),
        "a cancelled experiment offers no work"
    );
}

#[tokio::test]
async fn claim_leases_one_execution_and_starts_its_experiment() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let id = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");
    let worker = EvalWorkerId::new("worker-1");

    let claimed = f
        .experiments
        .claim(&f.owner, &worker)
        .await
        .expect("claim")
        .expect("an execution is queued");
    assert_eq!(claimed.experiment_id, id);
    assert_eq!(claimed.status, ExecutionStatus::Running);
    assert_eq!(claimed.fencing_token, 1, "the lease is fenced from one");
    assert!(claimed.lease_expires_at.is_some());

    let detail = f.experiments.get(&f.owner, &id).await.expect("get");
    assert_eq!(detail.experiment.status, ExperimentStatus::Running);

    let second = f
        .experiments
        .claim(&f.owner, &worker)
        .await
        .expect("claim")
        .expect("the second variant's execution is queued");
    assert_ne!(second.id, claimed.id);
    assert!(
        f.experiments
            .claim(&f.owner, &worker)
            .await
            .expect("claim")
            .is_none(),
        "the queue is drained while both executions are leased"
    );
    assert_eq!(
        f.experiments
            .get(&f.owner, &id)
            .await
            .expect("get")
            .experiment
            .status,
        ExperimentStatus::Running,
        "a running execution keeps the experiment running"
    );

    assert!(
        f.experiments
            .claim(&new_owner(&pool).await, &worker)
            .await
            .expect("claim")
            .is_none(),
        "another owner sees no work"
    );
}

#[tokio::test]
async fn claim_rejects_an_unusable_worker_identity() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;

    for identity in [String::new(), "   ".to_owned(), "w".repeat(256)] {
        assert!(
            matches!(
                f.experiments
                    .claim(&f.owner, &EvalWorkerId::new(identity.clone()))
                    .await,
                Err(EvaluationError::InvalidSpec(_))
            ),
            "worker identity {identity:?} must be rejected"
        );
    }
}

#[tokio::test]
async fn claim_holds_the_owner_to_two_live_leases() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 3);
    f.experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");
    let worker = EvalWorkerId::new("worker-1");

    let first = f
        .experiments
        .claim(&f.owner, &worker)
        .await
        .expect("claim")
        .expect("first");
    let second = f
        .experiments
        .claim(&f.owner, &worker)
        .await
        .expect("claim")
        .expect("second");
    assert_ne!(
        first.id, second.id,
        "each claim leases a distinct execution"
    );

    assert!(
        f.experiments
            .claim(&f.owner, &worker)
            .await
            .expect("claim")
            .is_none(),
        "a third live lease for one owner must be refused"
    );
}

#[tokio::test]
async fn claim_reaps_expired_leases_before_handing_out_work() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 2);
    let id = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");
    let worker = EvalWorkerId::new("worker-1");

    let leased = f
        .experiments
        .claim(&f.owner, &worker)
        .await
        .expect("claim")
        .expect("first");
    sqlx::query(
        "UPDATE eval_executions SET lease_expires_at = NOW() - INTERVAL '1 minute' WHERE id = $1",
    )
    .bind(leased.id.as_str())
    .execute(&pool)
    .await
    .expect("expire the lease");

    let next = f
        .experiments
        .claim(&f.owner, &worker)
        .await
        .expect("claim")
        .expect("second");
    assert_ne!(next.id, leased.id);

    let detail = f.experiments.get(&f.owner, &id).await.expect("get");
    let reaped = detail
        .executions
        .iter()
        .find(|execution| execution.id == leased.id)
        .expect("expired execution");
    assert_eq!(reaped.status, ExecutionStatus::Error);
    assert!(
        reaped
            .result
            .as_ref()
            .expect("terminal result")
            .summary
            .contains("lease expired"),
        "the reaped execution records why it ended"
    );
    assert!(reaped.finished_at.is_some());
}

#[tokio::test]
async fn claim_completes_an_experiment_once_its_executions_are_terminal() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let id = f
        .experiments
        .create_with_budget(&f.owner, "key-1", &f.budget, &matrix)
        .await
        .expect("create");
    let worker = EvalWorkerId::new("worker-1");

    for _ in 0..2 {
        let leased = f
            .experiments
            .claim(&f.owner, &worker)
            .await
            .expect("claim")
            .expect("a queued execution");
        sqlx::query(
            "UPDATE eval_executions SET status = 'completed', finished_at = NOW() WHERE id = $1",
        )
        .bind(leased.id.as_str())
        .execute(&pool)
        .await
        .expect("finish the execution");
    }

    assert!(
        f.experiments
            .claim(&f.owner, &worker)
            .await
            .expect("claim")
            .is_none()
    );
    assert_eq!(
        f.experiments
            .get(&f.owner, &id)
            .await
            .expect("get")
            .experiment
            .status,
        ExperimentStatus::Completed,
        "an exhausted queue completes the running experiment"
    );
}

#[path = "repository_admission.rs"]
mod admission;

#[path = "campaign_completion.rs"]
mod campaign_completion;

#[path = "repository_campaign_dispatch.rs"]
mod campaign_dispatch;
