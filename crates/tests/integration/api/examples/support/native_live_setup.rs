//! Repository-composed native live gateway fixture setup.
//!
//! Every harness owns a freshly generated user id, so seeded experiments,
//! workers, revisions and sessions are namespaced per run and retained for
//! accounting inspection. Normal fixture shutdown revokes its worker.

//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_evaluation::experiments::records::ExecutionRecord;
use systemprompt_evaluation::experiments::resources::{
    CaseContent, Partition, ResourceContent, RubricContent, WeightedDimension,
};
use systemprompt_evaluation::experiments::{
    ClientKind, ExecutionMode, ExperimentSpec, Objective, VariantSpec, content_digest,
};
use systemprompt_evaluation::repository::experiments::{
    EvaluationRepositories, ExecutionLease, ExperimentRepository, ManagedWorkspaceRegistration,
    RevisionRepository, WorkerRecord, WorkerRepository,
};
use systemprompt_identifiers::{EvalExperimentId, ModelId, ProviderId, ResourceRevisionId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;


pub const PROVIDER: &str = "native-fixture";

pub struct Harness {
    pub native_model: String,
    pub pool: systemprompt_database::DbPool,
    pub pg: PgPool,
    pub owner: UserId,
    pub environment: String,
    pub worker: WorkerRecord,
    pub experiment: EvalExperimentId,
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
        assertions: vec!["response_present".to_owned()],
    }
}

fn workspace(marker: &str) -> (systemprompt_models::managed::RevisionBundle, String, usize) {
    let asset_digest = match marker {
        "bundle" => "1e6ed65d77d6364eeaed5a745ba5c4985ae2b700dd85d7cf7f027bdf294a33fc",
        "candidate" => "dda18a0e21ae47c53b4309434cbc02ae8bf764fa83a6defbb719431242722aa7",
        "configuration" => "b7d64a9221007dfd5390f7df6cd5b8f3ea4f82faa1237141e35ebf161f5511a1",
        _ => unreachable!("known test projection"),
    };
    let revision = format!("revision-{marker}");
    let files = serde_json::json!({
        "schema_version":1,
        "assembler_version":"managed-bundle-v1",
        "root":revision,
        "revisions":{(revision):{
            "schema_version":1,"snapshot_id":format!("snapshot-{marker}"),"parent_id":null,
            "files":{"file.txt":{"digest":asset_digest,"bytes":marker.len(),"media_type":"text/plain","executable":false}},
            "dependencies":{}
        }},
        "assets":{(asset_digest):marker.as_bytes()}
    });
    let files: systemprompt_models::managed::RevisionBundle =
        serde_json::from_value(files).expect("typed native workspace bundle");
    let digest = content_digest(&files).expect("workspace digest");
    (files, digest, marker.len())
}

impl Harness {
    pub async fn start(plan: &serde_json::Value) -> anyhow::Result<Self> {
        let repetitions = 1;
        let native_model = plan["model"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Native plan lacks model"))?;
        let native_version = plan["client_version"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Native plan lacks version"))?;
        let native_client: ClientKind = serde_json::from_value(plan["client"].clone())?;
        let native_image = plan["image_id"]
            .as_str()
            .and_then(|value| value.strip_prefix("sha256:"))
            .ok_or_else(|| anyhow::anyhow!("Native plan lacks exact image"))?;
        let url = fixture_database_url()?;
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let pg = (*pool.write_pool_arc().expect("write pool")).clone();
        let owner = unique_user_id("eval-owner");
        seed_user_row(&pool, &owner, &format!("{}@example.test", owner.as_str()))
            .await
            .expect("seed owner");

        let evidence = repositories(&pool, fixture_admission())?.evidence;
        let (bundle, bundle_digest, bundle_bytes) = workspace("bundle");
        let (candidate, candidate_digest, candidate_bytes) = workspace("candidate");
        let (configuration, configuration_digest, configuration_bytes) = workspace("configuration");
        for (revision, manifest, digest, bytes) in [
            ("revision-bundle", &bundle, &bundle_digest, bundle_bytes),
            (
                "revision-candidate",
                &candidate,
                &candidate_digest,
                candidate_bytes,
            ),
            (
                "revision-configuration",
                &configuration,
                &configuration_digest,
                configuration_bytes,
            ),
        ] {
            evidence
                .register_managed_workspace(
                    &owner,
                    &ManagedWorkspaceRegistration {
                        managed_revision_id: &ResourceRevisionId::new(revision),
                        publication_generation: Some(1),
                        manifest,
                        expected_digest: digest,
                        file_count: 1,
                        byte_count: bytes,
                    },
                )
                .await
                .expect("managed projection");
        }

        let revisions = RevisionRepository::new(pg.clone());
        let rubric_revision = revisions
            .create(&owner, "rubric", &ResourceContent::Rubric(rubric_content()))
            .await
            .expect("rubric revision");
        let case_revision = revisions
            .create(&owner, "case", &ResourceContent::Case(case_content()))
            .await
            .expect("case revision");
        let dataset_content = ResourceContent::Dataset(vec![case_revision.clone()]);
        let dataset_revision = revisions
            .create(&owner, "dataset", &dataset_content)
            .await
            .expect("dataset revision");

        let mut frozen: systemprompt_evaluation::experiments::FrozenSettings =
            serde_json::from_value(plan["frozen"].clone())?;
        frozen.cost_envelope.generation_microdollars_per_attempt = 250_000;
        frozen.cost_envelope.judging_microdollars_per_attempt = 250_000;
        frozen.dataset_digest = content_digest(&dataset_content)?;
        frozen.rubric_digest = content_digest(&ResourceContent::Rubric(rubric_content()))?;
        let spec = ExperimentSpec {
            schema_version: 1,
            name: "harness".to_owned(),
            cases: vec![case_revision.clone()],
            rubric: rubric_revision.clone(),
            dataset: Some(dataset_revision),
            variants: vec![
                VariantSpec {
                    client: native_client,
                    client_version: native_version.to_owned(),
                    model: ModelId::new(native_model),
                    provider: ProviderId::new(PROVIDER),
                    skill_bundle_digest: bundle_digest.clone(),
                    configuration_digest: configuration_digest.clone(),
                    worker_image_digest: native_image.to_owned(),
                },
                VariantSpec {
                    client: native_client,
                    client_version: native_version.to_owned(),
                    model: ModelId::new(native_model),
                    provider: ProviderId::new(PROVIDER),
                    skill_bundle_digest: candidate_digest,
                    configuration_digest: configuration_digest.clone(),
                    worker_image_digest: native_image.to_owned(),
                },
            ],
            repetitions,
            budget_microdollars: i64::from(repetitions) * 1_000_000,
            execution_mode: ExecutionMode::Fixture,
            objective: Objective::Quality,
            frozen: Some(frozen),
            claim_independent_improvement: false,
        };
        let budget = repositories(&pool, fixture_admission())?
            .budgets
            .create_shared(&owner, &format!("budget-{}", Uuid::new_v4()), 5_000_000)
            .await
            .expect("shared budget");
        let experiment = repositories(&pool, fixture_admission())?
            .experiments
            .create_with_budget(&owner, &format!("key-{}", Uuid::new_v4()), &budget, &spec)
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

        Ok(Self {
            native_model: native_model.to_owned(),
            pool,
            pg,
            owner,
            environment,
            worker,
            experiment,
        })
    }

    pub fn repositories(
        &self,
        admission: Arc<dyn systemprompt_evaluation::capabilities::ExecutionAdmission>,
    ) -> anyhow::Result<EvaluationRepositories> {
        repositories(&self.pool, admission)
    }

    pub fn experiments(&self) -> ExperimentRepository {
        self.repositories(fixture_admission())
            .expect("evaluation repositories")
            .experiments
    }

    pub fn workers(&self) -> WorkerRepository {
        WorkerRepository::new(self.pg.clone())
    }

    pub async fn claimed_lease(&self) -> anyhow::Result<(ExecutionRecord, ExecutionLease)> {
        let execution = self
            .experiments()
            .claim(&self.owner, &self.worker.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No fixture execution was queued"))?;
        let lease = ExecutionLease::builder(execution.id.clone(), self.worker.id.clone())
            .fencing_token(execution.fencing_token)
            .build()?;
        Ok((execution, lease))
    }
}

#[derive(Debug, Clone, Copy)]
struct NativeFixtureAdmission;
impl systemprompt_evaluation::capabilities::ExecutionAdmission for NativeFixtureAdmission {
    fn admit(&self, spec: &ExperimentSpec) -> systemprompt_evaluation::Result<()> {
        spec.validate()?;
        systemprompt_evaluation::capabilities::paired_variants(spec)?;
        if spec.execution_mode != ExecutionMode::Fixture {
            return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
                "Native fixture admits deterministic provider work only".to_owned(),
            ));
        }
        Ok(())
    }
}
pub fn fixture_admission()
-> std::sync::Arc<dyn systemprompt_evaluation::capabilities::ExecutionAdmission> {
    std::sync::Arc::new(NativeFixtureAdmission)
}

fn repositories(
    pool: &systemprompt_database::DbPool,
    admission: Arc<dyn systemprompt_evaluation::capabilities::ExecutionAdmission>,
) -> anyhow::Result<EvaluationRepositories> {
    Ok(EvaluationRepositories::with_admission(
        pool,
        systemprompt_test_fixtures::fixture_evaluation_seams(pool)?,
        admission,
    )?)
}
