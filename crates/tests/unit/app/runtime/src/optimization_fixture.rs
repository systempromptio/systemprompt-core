//! Real managed revisions and deterministic campaign inputs for runtime tests.
use std::collections::BTreeMap;
use std::sync::Arc;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_evaluation::capabilities::ExecutionAdmission;
use systemprompt_evaluation::experiments::resources::{
    CaseContent, Partition, ResourceContent, RubricContent, WeightedDimension,
};
use systemprompt_evaluation::experiments::{
    ClientKind, ExecutionMode, ExperimentSpec, FrozenCostEnvelope, FrozenSettings, Objective,
    VariantSpec, content_digest,
};
use systemprompt_evaluation::repository::experiments::EvaluationRepositories;
use systemprompt_identifiers::{
    EvalCampaignId, ManagedResourceId, ModelId, ProviderId, ResourceRevisionId, TraceId, UserId,
};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ManagedRepository, NewResource, NewRevision, ResourceKind,
    RevisionFiles, SnapshotProvenance, SourceSpec, TextCandidate,
};
use systemprompt_runtime::optimization::SkillOptimizationOrchestrator;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

#[derive(Debug)]
struct FixtureAdmission;
impl ExecutionAdmission for FixtureAdmission {
    fn admit(&self, spec: &ExperimentSpec) -> systemprompt_evaluation::Result<()> {
        spec.validate()?;
        if spec.execution_mode != ExecutionMode::Fixture {
            return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
                "Only deterministic fixture execution is admitted".to_owned(),
            ));
        }
        Ok(())
    }
}

pub(super) struct Fixture {
    pub(super) db: systemprompt_database::DbPool,
    pub(super) owner: UserId,
    pub(super) managed: ManagedRepository,
    pub(super) repositories: EvaluationRepositories,
    pub(super) runtime: SkillOptimizationOrchestrator,
    pub(super) campaign: EvalCampaignId,
    pub(super) policy: CampaignPolicy,
    pub(super) spec: ExperimentSpec,
}
impl Fixture {
    pub(super) async fn new() -> Self {
        let bootstrap = ensure_test_bootstrap();
        let db = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("database");
        let pool = db.write_pool_arc().expect("write pool").as_ref().clone();
        let owner = UserId::new(format!("optimization-{}", TraceId::generate()));
        seed_user_row(&db, &owner, &format!("{owner}@optimization.invalid"))
            .await
            .expect("owner");
        let managed = ManagedRepository::new(pool.clone());
        let (resource, baseline) = resource(&managed, &owner, "baseline").await;
        let candidate = managed
            .create_text_candidate(
                &owner,
                &baseline,
                &TextCandidate {
                    path: "index.md".to_owned(),
                    content: "# improved".to_owned(),
                    rationale: "improved candidate".to_owned(),
                },
            )
            .await
            .expect("candidate");
        let repositories = EvaluationRepositories::with_admission(
            &db,
            systemprompt_test_fixtures::fixture_evaluation_seams(&db).expect("evaluation seams"),
            Arc::new(FixtureAdmission),
        )
        .expect("evaluation repositories");
        let runtime = SkillOptimizationOrchestrator::new(
            managed.clone(),
            repositories.clone(),
            repositories.revisions.clone(),
        );
        let baseline_digest = runtime
            .register_workspace(&owner, &baseline)
            .await
            .expect("baseline workspace");
        let candidate_digest = runtime
            .register_workspace(&owner, &candidate)
            .await
            .expect("candidate workspace");
        let case = repositories
            .revisions
            .create(
                &owner,
                "case",
                &ResourceContent::Case(CaseContent {
                    prompt: "Write an independently checkable explanation".to_owned(),
                    expected_behavior: vec!["Response contains evidence".to_owned()],
                    fixtures: BTreeMap::new(),
                    partition: Partition::Development,
                    assertions: vec!["response_present".to_owned()],
                }),
            )
            .await
            .expect("case");
        let rubric_content = ResourceContent::Rubric(RubricContent {
            dimensions: vec![WeightedDimension {
                name: "quality".to_owned(),
                description: "Evidence quality".to_owned(),
                weight: 1,
            }],
            pass_threshold_milli: 3000,
            hard_gates: vec![],
        });
        let rubric = repositories
            .revisions
            .create(&owner, "rubric", &rubric_content)
            .await
            .expect("rubric");
        let dataset_content = ResourceContent::Dataset(vec![case.clone()]);
        let dataset = repositories
            .revisions
            .create(&owner, "dataset", &dataset_content)
            .await
            .expect("dataset");
        let budget = repositories
            .budgets
            .create_shared(&owner, "campaign-budget", 1000)
            .await
            .expect("budget");
        let policy = CampaignPolicy {
            name: "Runtime campaign".to_owned(),
            resource_id: resource,
            baseline_revision_id: baseline,
            budget_id: budget,
            objective: OptimizationObjective::Quality,
            minimum_quality_milli: 3000,
            minimum_pairs: 2,
            maximum_iterations: 2,
            automatic: true,
        };
        let campaign = repositories
            .campaigns
            .create(&owner, &owner, "campaign", &policy)
            .await
            .expect("campaign");
        let mut spec = ExperimentSpec {
            schema_version: 1,
            name: "runtime pair".to_owned(),
            cases: vec![case],
            rubric,
            dataset: Some(dataset),
            variants: vec![variant(baseline_digest), variant(candidate_digest)],
            repetitions: 2,
            budget_microdollars: 4,
            execution_mode: ExecutionMode::Fixture,
            objective: Objective::Quality,
            frozen: Some(FrozenSettings {
                provider_prices_digest: "e".repeat(64),
                tool_configuration_digest: "f".repeat(64),
                fixture_clock: "2026-09-12T08:00:00Z".to_owned(),
                fixture_timezone: "UTC".to_owned(),
                permissions_digest: "1".repeat(64),
                dataset_digest: content_digest(&dataset_content).expect("dataset digest"),
                rubric_digest: content_digest(&rubric_content).expect("rubric digest"),
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
        };
        let configuration = spec.variants[0].skill_bundle_digest.clone();
        for variant in &mut spec.variants {
            variant.configuration_digest = configuration.clone();
        }
        Self {
            db,
            owner,
            managed,
            repositories,
            runtime,
            campaign,
            policy,
            spec,
        }
    }
}
fn variant(digest: String) -> VariantSpec {
    VariantSpec {
        client: ClientKind::ClaudeCode,
        client_version: "1.0.0".to_owned(),
        model: ModelId::new("claude-sonnet-5"),
        provider: ProviderId::new("anthropic"),
        skill_bundle_digest: digest,
        configuration_digest: "b".repeat(64),
        worker_image_digest: "c".repeat(64),
    }
}
pub(super) async fn resource(
    managed: &ManagedRepository,
    owner: &UserId,
    key: &str,
) -> (ManagedResourceId, ResourceRevisionId) {
    let source = managed
        .register_source(owner, key, &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = managed
        .capture_snapshot(
            owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(key.as_bytes()),
                importer_version: "runtime-fixture".to_owned(),
            },
        )
        .await
        .expect("snapshot");
    let resource = managed
        .bind_resource(
            owner,
            &NewResource {
                source_id: source,
                upstream_key: key.to_owned(),
                kind: ResourceKind::Skill,
                resource_key: key.to_owned(),
            },
        )
        .await
        .expect("resource");
    let files = RevisionFiles(BTreeMap::from([(
        "index.md".to_owned(),
        AssetFile {
            bytes: b"# baseline".to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    )]));
    let revision = managed
        .create_revision(
            owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files,
                dependencies: BTreeMap::new(),
                rationale: "baseline".to_owned(),
            },
        )
        .await
        .expect("revision");
    (resource, revision)
}
