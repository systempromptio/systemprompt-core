//! DB-backed tests for the evidence repository: frozen workspace storage,
//! fenced evidence submission and the owner-scoped evidence reads. Each test
//! seeds its own UUID owner, revisions and experiment, so the lease and
//! immutability assertions never observe another test's rows.

use sqlx::PgPool;
use std::collections::BTreeMap;
use systemprompt_evaluation::EvaluationError;
use systemprompt_evaluation::experiments::execution::{
    ArtifactEvidence, ClientCapabilities, ExecutionEvidence, FrozenWorkspace,
};
use systemprompt_evaluation::experiments::resources::{
    CaseContent, Partition, ResourceContent, RubricContent, WeightedDimension,
};
use systemprompt_evaluation::experiments::{
    ClientKind, ExecutionMode, ExperimentSpec, Objective, VariantSpec,
};
use systemprompt_evaluation::repository::experiments::{
    EvidenceRepository, ExecutionLease, ExperimentRepository, RevisionRepository,
};
use systemprompt_identifiers::{
    AiRequestId, EvalExecutionId, EvalRevisionId, EvalWorkerId, ModelId, ProviderId, UserId,
};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

const BUNDLE_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const IMAGE_DIGEST: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const REPORT_BODY: &str = "findings";
const REPORT_SHA256: &str = "8aa3c4ce3ccdfe89f32935d164b3fb04ea01afa7a71f17c5be4f777cb7968cbc";

async fn evidence_pool() -> Option<PgPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let write = pool.write_pool_arc().expect("write pool");
    Some(write.as_ref().clone())
}

fn new_owner() -> UserId {
    UserId::new(format!("eval-evidence-{}", Uuid::new_v4()))
}

fn workspace(files: &[(&str, &str)]) -> FrozenWorkspace {
    FrozenWorkspace {
        files: files
            .iter()
            .map(|(path, body)| ((*path).to_owned(), (*body).to_owned()))
            .collect(),
    }
}

fn capabilities() -> ClientCapabilities {
    ClientCapabilities {
        client: ClientKind::ClaudeCode,
        client_version: "1.0.0".to_owned(),
        adapter_version: "0.1.0".to_owned(),
        image_digest: IMAGE_DIGEST.to_owned(),
        supports_session_resume: false,
    }
}

fn variant() -> VariantSpec {
    VariantSpec {
        client: ClientKind::ClaudeCode,
        client_version: "1.0.0".to_owned(),
        model: ModelId::new("claude-sonnet-5"),
        provider: ProviderId::new("anthropic"),
        skill_bundle_digest: BUNDLE_DIGEST.to_owned(),
        configuration_digest: "b".repeat(64),
        worker_image_digest: IMAGE_DIGEST.to_owned(),
    }
}

fn spec(case: EvalRevisionId, rubric: EvalRevisionId) -> ExperimentSpec {
    ExperimentSpec {
        schema_version: 1,
        name: "evidence".to_owned(),
        cases: vec![case],
        rubric,
        variants: vec![variant()],
        repetitions: 1,
        budget_microdollars: 5_000_000,
        execution_mode: ExecutionMode::Fixture,
        objective: Objective::Quality,
    }
}

struct Fixture {
    evidence: EvidenceRepository,
    owner: UserId,
    lease: ExecutionLease,
}

async fn fixture(pool: &PgPool) -> Fixture {
    let owner = new_owner();
    let revisions = RevisionRepository::new(pool.clone());
    let case = revisions
        .create(
            &owner,
            "case-key",
            &ResourceContent::Case(CaseContent {
                prompt: "Write a specification".to_owned(),
                expected_behavior: vec!["Cites its sources".to_owned()],
                fixtures: BTreeMap::new(),
                partition: Partition::Development,
            }),
        )
        .await
        .expect("case revision");
    let rubric = revisions
        .create(
            &owner,
            "rubric-key",
            &ResourceContent::Rubric(RubricContent {
                dimensions: vec![WeightedDimension {
                    name: "grounding".to_owned(),
                    description: "Claims are supported".to_owned(),
                    weight: 1,
                }],
                pass_threshold_milli: 3000,
                hard_gates: Vec::new(),
            }),
        )
        .await
        .expect("rubric revision");
    let experiments = ExperimentRepository::new(pool.clone());
    experiments
        .create(&owner, "key-1", &spec(case, rubric))
        .await
        .expect("experiment");
    let worker = EvalWorkerId::new("worker-1");
    let execution = experiments
        .claim(&owner, &worker)
        .await
        .expect("claim")
        .expect("an execution is queued");
    let lease = ExecutionLease::builder(execution.id, worker)
        .fencing_token(execution.fencing_token)
        .build()
        .expect("lease");
    Fixture {
        evidence: EvidenceRepository::new(pool.clone()),
        owner,
        lease,
    }
}

fn evidence_for(lease: &ExecutionLease, elapsed: u64) -> ExecutionEvidence {
    ExecutionEvidence {
        execution_id: lease.execution_id.clone(),
        fencing_token: lease.fencing_token,
        capabilities: capabilities(),
        installed_bundle_digest: BUNDLE_DIGEST.to_owned(),
        candidate_bundle_digest: BUNDLE_DIGEST.to_owned(),
        workspace_digest: workspace(&[]).digest().expect("digest"),
        requests: Vec::new(),
        artifacts: Vec::new(),
        exit_code: Some(0),
        elapsed_milliseconds: elapsed,
        cleanup_confirmed: true,
    }
}

#[tokio::test]
async fn workspaces_are_stored_once_and_read_back_in_scope() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let evidence = EvidenceRepository::new(pool.clone());
    let owner = new_owner();
    let frozen = workspace(&[("src/main.rs", "fn main() {}")]);

    let digest = evidence
        .save_workspace(&owner, &frozen)
        .await
        .expect("save");
    assert_eq!(
        evidence
            .save_workspace(&owner, &frozen)
            .await
            .expect("save"),
        digest,
        "re-freezing identical content is a no-op returning the same digest"
    );

    let loaded = evidence
        .get_workspace(&owner, &digest)
        .await
        .expect("get workspace");
    assert_eq!(loaded.files, frozen.files);

    assert!(matches!(
        evidence.get_workspace(&owner, &"f".repeat(64)).await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(
        matches!(
            evidence.get_workspace(&new_owner(), &digest).await,
            Err(EvaluationError::ResourceNotFound(_))
        ),
        "frozen workspaces are owner-scoped"
    );
}

#[tokio::test]
async fn a_workspace_stored_under_the_wrong_digest_is_refused_on_read() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let evidence = EvidenceRepository::new(pool.clone());
    let owner = new_owner();
    let claimed = "d".repeat(64);

    sqlx::query(
        "INSERT INTO eval_frozen_workspaces(owner_id, digest, content) VALUES ($1, $2, $3)",
    )
    .bind(owner.as_str())
    .bind(&claimed)
    .bind(serde_json::to_value(workspace(&[("a.txt", "tampered")])).expect("json"))
    .execute(&pool)
    .await
    .expect("seed tampered workspace");

    assert!(matches!(
        evidence.get_workspace(&owner, &claimed).await,
        Err(EvaluationError::InvalidSpec(_))
    ));
}

#[tokio::test]
async fn submitted_evidence_is_readable_and_immutable() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let artifacts = workspace(&[("report.md", REPORT_BODY)]);
    let manifest = ArtifactEvidence {
        relative_path: "report.md".to_owned(),
        sha256: REPORT_SHA256.to_owned(),
        bytes: REPORT_BODY.len() as u64,
    };
    let mut evidence = evidence_for(&f.lease, 1_200);
    evidence.artifacts = vec![manifest];
    evidence.workspace_digest = artifacts.digest().expect("digest");

    f.evidence
        .submit(&f.owner, &f.lease, &evidence, &artifacts)
        .await
        .expect("submit");

    let stored = f
        .evidence
        .get(&f.owner, &f.lease.execution_id)
        .await
        .expect("get");
    assert_eq!(stored.execution_id, f.lease.execution_id);
    assert_eq!(stored.fencing_token, f.lease.fencing_token);
    assert_eq!(stored.elapsed_milliseconds, 1_200);
    assert_eq!(stored.artifacts.len(), 1);
    assert_eq!(
        f.evidence
            .get_artifacts(&f.owner, &f.lease.execution_id)
            .await
            .expect("artifacts")
            .files,
        artifacts.files
    );

    f.evidence
        .submit(&f.owner, &f.lease, &evidence, &artifacts)
        .await
        .expect("an identical resubmission is accepted as a replay");

    let mut altered = evidence;
    altered.elapsed_milliseconds = 9_999;
    assert!(
        matches!(
            f.evidence
                .submit(&f.owner, &f.lease, &altered, &artifacts)
                .await,
            Err(EvaluationError::ExperimentConflict(_))
        ),
        "recorded evidence cannot be rewritten"
    );
}

#[tokio::test]
async fn evidence_reads_are_owner_scoped() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let artifacts = workspace(&[]);
    let evidence = evidence_for(&f.lease, 10);
    f.evidence
        .submit(&f.owner, &f.lease, &evidence, &artifacts)
        .await
        .expect("submit");

    let stranger = new_owner();
    assert!(matches!(
        f.evidence.get(&stranger, &f.lease.execution_id).await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(matches!(
        f.evidence
            .get_artifacts(&stranger, &f.lease.execution_id)
            .await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(matches!(
        f.evidence.get(&f.owner, &EvalExecutionId::generate()).await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
    assert!(matches!(
        f.evidence
            .get_artifacts(&f.owner, &EvalExecutionId::generate())
            .await,
        Err(EvaluationError::ResourceNotFound(_))
    ));
}

#[tokio::test]
async fn artifact_payloads_must_match_their_manifest() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let manifest = ArtifactEvidence {
        relative_path: "report.md".to_owned(),
        sha256: REPORT_SHA256.to_owned(),
        bytes: REPORT_BODY.len() as u64,
    };

    let mut declared = evidence_for(&f.lease, 10);
    declared.artifacts = vec![manifest.clone()];

    assert!(
        matches!(
            f.evidence
                .submit(&f.owner, &f.lease, &declared, &workspace(&[]))
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a manifest entry with no payload is refused"
    );
    assert!(
        matches!(
            f.evidence
                .submit(
                    &f.owner,
                    &f.lease,
                    &declared,
                    &workspace(&[("elsewhere.md", REPORT_BODY)])
                )
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "the payload must be carried under the manifest path"
    );
    assert!(
        matches!(
            f.evidence
                .submit(
                    &f.owner,
                    &f.lease,
                    &declared,
                    &workspace(&[("report.md", "different findings")])
                )
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "content must match its declared hash and size"
    );
    assert!(
        matches!(
            f.evidence
                .submit(
                    &f.owner,
                    &f.lease,
                    &evidence_for(&f.lease, 10),
                    &workspace(&[("report.md", REPORT_BODY)])
                )
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "an unmanifested payload file is refused"
    );

    let mut unfenced = evidence_for(&f.lease, 10);
    unfenced.fencing_token = 0;
    assert!(
        matches!(
            f.evidence
                .submit(&f.owner, &f.lease, &unfenced, &workspace(&[]))
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "evidence is self-validated before the lease is consulted"
    );
}

#[tokio::test]
async fn evidence_must_carry_the_lease_it_was_issued_under() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let artifacts = workspace(&[]);

    let mut other_execution = evidence_for(&f.lease, 10);
    other_execution.execution_id = EvalExecutionId::generate();
    assert!(matches!(
        f.evidence
            .submit(&f.owner, &f.lease, &other_execution, &artifacts)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));

    let mut other_token = evidence_for(&f.lease, 10);
    other_token.fencing_token = f.lease.fencing_token + 1;
    assert!(matches!(
        f.evidence
            .submit(&f.owner, &f.lease, &other_token, &artifacts)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));

    let foreign = ExecutionLease::builder(
        f.lease.execution_id.clone(),
        EvalWorkerId::new("another-worker"),
    )
    .fencing_token(f.lease.fencing_token)
    .build()
    .expect("lease");
    assert!(
        matches!(
            f.evidence
                .submit(&f.owner, &foreign, &evidence_for(&foreign, 10), &artifacts)
                .await,
            Err(EvaluationError::ExperimentConflict(_))
        ),
        "another worker cannot submit against this execution"
    );
    assert!(
        matches!(
            f.evidence
                .submit(
                    &new_owner(),
                    &f.lease,
                    &evidence_for(&f.lease, 10),
                    &artifacts
                )
                .await,
            Err(EvaluationError::ExperimentConflict(_))
        ),
        "another owner cannot submit against this execution"
    );
}

#[tokio::test]
async fn an_expired_lease_cannot_submit_evidence() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    sqlx::query(
        "UPDATE eval_executions SET lease_expires_at = NOW() - INTERVAL '1 minute' WHERE id = $1",
    )
    .bind(f.lease.execution_id.as_str())
    .execute(&pool)
    .await
    .expect("expire the lease");

    assert!(matches!(
        f.evidence
            .submit(
                &f.owner,
                &f.lease,
                &evidence_for(&f.lease, 10),
                &workspace(&[])
            )
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
    assert!(
        matches!(
            f.evidence.get(&f.owner, &f.lease.execution_id).await,
            Err(EvaluationError::ResourceNotFound(_))
        ),
        "the rejected submission stored nothing"
    );
}

#[tokio::test]
async fn evidence_must_match_the_frozen_variant() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let artifacts = workspace(&[]);

    let mut swapped_bundle = evidence_for(&f.lease, 10);
    swapped_bundle.candidate_bundle_digest = "e".repeat(64);
    assert!(matches!(
        f.evidence
            .submit(&f.owner, &f.lease, &swapped_bundle, &artifacts)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));

    let mut swapped_image = evidence_for(&f.lease, 10);
    swapped_image.capabilities.image_digest = "e".repeat(64);
    assert!(matches!(
        f.evidence
            .submit(&f.owner, &f.lease, &swapped_image, &artifacts)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));

    let mut swapped_version = evidence_for(&f.lease, 10);
    swapped_version.capabilities.client_version = "9.9.9".to_owned();
    assert!(matches!(
        f.evidence
            .submit(&f.owner, &f.lease, &swapped_version, &artifacts)
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
}

#[tokio::test]
async fn evidence_request_ids_must_match_the_server_audit_trail() {
    let Some(pool) = evidence_pool().await else {
        return;
    };
    let f = fixture(&pool).await;
    let mut invented = evidence_for(&f.lease, 10);
    invented.requests = vec![AiRequestId::new(format!("eval-req-{}", Uuid::new_v4()))];

    assert!(
        matches!(
            f.evidence
                .submit(&f.owner, &f.lease, &invented, &workspace(&[]))
                .await,
            Err(EvaluationError::ExperimentConflict(_))
        ),
        "a request the gateway never admitted cannot enter the manifest"
    );
}
