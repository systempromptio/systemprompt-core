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
    ClientKind, ExecutionMode, ExperimentSpec, Objective, VariantSpec,
};
use systemprompt_evaluation::repository::experiments::{ExperimentRepository, RevisionRepository};
use systemprompt_identifiers::{
    EvalExperimentId, EvalRevisionId, EvalWorkerId, ModelId, ProviderId, UserId,
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

fn new_owner() -> UserId {
    UserId::new(format!("eval-runs-{}", Uuid::new_v4()))
}

fn case_content(prompt: &str) -> ResourceContent {
    ResourceContent::Case(CaseContent {
        prompt: prompt.to_owned(),
        expected_behavior: vec!["Cites its sources".to_owned()],
        fixtures: BTreeMap::new(),
        partition: Partition::Development,
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

fn variant(version: &str) -> VariantSpec {
    VariantSpec {
        client: ClientKind::ClaudeCode,
        client_version: version.to_owned(),
        model: ModelId::new("claude-sonnet-5"),
        provider: ProviderId::new("anthropic"),
        skill_bundle_digest: "a".repeat(64),
        configuration_digest: "b".repeat(64),
        worker_image_digest: "c".repeat(64),
    }
}

fn spec(cases: Vec<EvalRevisionId>, rubric: EvalRevisionId, repetitions: u32) -> ExperimentSpec {
    ExperimentSpec {
        schema_version: 1,
        name: "matrix".to_owned(),
        cases,
        rubric,
        variants: vec![variant("1.0.0")],
        repetitions,
        budget_microdollars: 5_000_000,
        execution_mode: ExecutionMode::Fixture,
        objective: Objective::Quality,
    }
}

struct Fixture {
    experiments: ExperimentRepository,
    owner: UserId,
    case: EvalRevisionId,
    rubric: EvalRevisionId,
}

async fn fixture(pool: &PgPool) -> Fixture {
    let owner = new_owner();
    let revisions = RevisionRepository::new(pool.clone());
    let case = revisions
        .create(&owner, "case-key", &case_content("Write a specification"))
        .await
        .expect("case revision");
    let rubric = revisions
        .create(&owner, "rubric-key", &rubric_content())
        .await
        .expect("rubric revision");
    Fixture {
        experiments: ExperimentRepository::new(pool.clone()),
        owner,
        case,
        rubric,
    }
}

#[tokio::test]
async fn create_freezes_the_full_execution_matrix() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let mut matrix = spec(vec![f.case.clone()], f.rubric.clone(), 2);
    matrix.variants.push(variant("2.0.0"));

    let id = f
        .experiments
        .create(&f.owner, "key-1", &matrix)
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
    let first_spec = spec(vec![f.case.clone()], f.rubric.clone(), 1);

    let id = f
        .experiments
        .create(&f.owner, "key-1", &first_spec)
        .await
        .expect("create");
    let repeated = f
        .experiments
        .create(&f.owner, "key-1", &first_spec)
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
        1,
        "the replay must not duplicate executions"
    );

    let mut changed = first_spec;
    changed.name = "different".to_owned();
    assert!(matches!(
        f.experiments.create(&f.owner, "key-1", &changed).await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
}

#[tokio::test]
async fn create_rejects_invalid_keys_and_mistyped_resource_references() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let valid = spec(vec![f.case.clone()], f.rubric.clone(), 1);

    for key in ["", "   "] {
        assert!(
            matches!(
                f.experiments.create(&f.owner, key, &valid).await,
                Err(EvaluationError::InvalidSpec(_))
            ),
            "key {key:?} must be rejected"
        );
    }
    let long_key = "k".repeat(256);
    assert!(matches!(
        f.experiments.create(&f.owner, &long_key, &valid).await,
        Err(EvaluationError::InvalidSpec(_))
    ));

    let mut empty_matrix = valid.clone();
    empty_matrix.variants.clear();
    assert!(
        matches!(
            f.experiments.create(&f.owner, "key-2", &empty_matrix).await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "the spec is validated before anything is persisted"
    );

    let swapped = spec(vec![f.rubric.clone()], f.case.clone(), 1);
    assert!(
        matches!(
            f.experiments.create(&f.owner, "key-3", &swapped).await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a case revision cannot stand in for the rubric"
    );

    let case_is_rubric = spec(vec![f.rubric.clone()], f.rubric.clone(), 1);
    assert!(
        matches!(
            f.experiments
                .create(&f.owner, "key-4", &case_is_rubric)
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a rubric revision cannot stand in for a case"
    );

    let foreign = spec(vec![EvalRevisionId::generate()], f.rubric.clone(), 1);
    assert!(matches!(
        f.experiments.create(&f.owner, "key-5", &foreign).await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
}

#[tokio::test]
async fn reads_are_owner_scoped_and_listed_newest_first() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = spec(vec![f.case.clone()], f.rubric.clone(), 1);

    let first = f
        .experiments
        .create(&f.owner, "key-1", &matrix)
        .await
        .expect("create");
    sqlx::query("UPDATE eval_experiments SET created_at = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(first.as_str())
        .execute(&pool)
        .await
        .expect("age the first experiment");
    let second = f
        .experiments
        .create(&f.owner, "key-2", &matrix)
        .await
        .expect("create");

    let listed = f.experiments.list(&f.owner).await.expect("list");
    let ids: Vec<&EvalExperimentId> = listed.iter().map(|record| &record.id).collect();
    assert_eq!(ids, vec![&second, &first], "newest first");
    assert!(listed.iter().all(|record| record.owner_id == f.owner));

    let stranger = new_owner();
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
async fn cancellation_freezes_the_budget_and_drains_the_queue() {
    let Some(pool) = runs_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let matrix = spec(vec![f.case.clone()], f.rubric.clone(), 2);
    let id = f
        .experiments
        .create(&f.owner, "key-1", &matrix)
        .await
        .expect("create");

    assert!(
        matches!(
            f.experiments.cancel(&new_owner(), &id).await,
            Err(EvaluationError::ExperimentConflict(_))
        ),
        "cancellation is owner-scoped"
    );

    f.experiments.cancel(&f.owner, &id).await.expect("cancel");
    let detail = f.experiments.get(&f.owner, &id).await.expect("get");
    assert_eq!(detail.experiment.status, ExperimentStatus::Cancelled);
    assert!(
        detail.experiment.accounting.frozen,
        "a cancelled experiment must admit no further spend"
    );
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
    let matrix = spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let id = f
        .experiments
        .create(&f.owner, "key-1", &matrix)
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

    assert!(
        f.experiments
            .claim(&f.owner, &worker)
            .await
            .expect("claim")
            .is_none(),
        "the queue is drained while the single execution is leased"
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
            .claim(&new_owner(), &worker)
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
    let matrix = spec(vec![f.case.clone()], f.rubric.clone(), 3);
    f.experiments
        .create(&f.owner, "key-1", &matrix)
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
    let matrix = spec(vec![f.case.clone()], f.rubric.clone(), 2);
    let id = f
        .experiments
        .create(&f.owner, "key-1", &matrix)
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
    let matrix = spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let id = f
        .experiments
        .create(&f.owner, "key-1", &matrix)
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
        "UPDATE eval_executions SET status = 'completed', finished_at = NOW() WHERE id = $1",
    )
    .bind(leased.id.as_str())
    .execute(&pool)
    .await
    .expect("finish the execution");

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
