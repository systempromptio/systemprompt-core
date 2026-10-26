// Publication gates run before the publication transaction: a required gate
// with none registered refuses, a refusal refuses, and an admission is
// recorded on the review. Cohort rings resolve a consumer through the first
// matching ring with a cohort selection, and the catalogue stamp moves with
// cohort generations.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_governance::authz::{
    AuthzHookContext, NullAuditSink, SubjectProviderSet, discover_role_directory,
};
use systemprompt_identifiers::{ResourceRevisionId, UserId};
use systemprompt_manifest::services::{
    CohortRing, ManagedMarketplaceConfig, PublicationConfig, PublicationGateConfig,
};
use systemprompt_marketplace::managed::{
    AssetDigest, CohortSelector, ComparisonEvidence, GateContext, GateVerdict, ManagedError,
    ManagedRepository, ManagedResolution, NewRevision, PublicationAction, PublicationDecision,
    PublicationGate, PublicationGateError, PublicationPolicy, PublicationRequest, ResourceKind,
};
use systemprompt_test_fixtures::{seed_user_row_with_roles, test_db_pool};
use systemprompt_users as _;
use uuid::Uuid;

use crate::managed_resolution::{Fixture, fixture, publish, skill_files};

struct FixedGate(GateVerdict);

#[async_trait]
impl PublicationGate for FixedGate {
    fn id(&self) -> &'static str {
        "fixed"
    }

    async fn admit(&self, _ctx: &GateContext<'_>) -> Result<GateVerdict, PublicationGateError> {
        Ok(self.0.clone())
    }
}

fn gated(
    f: &Fixture,
    gate: PublicationGateConfig,
    gates: Vec<Arc<dyn PublicationGate>>,
) -> ManagedRepository {
    f.repository
        .clone()
        .with_publication_policy(PublicationPolicy::new(
            ManagedMarketplaceConfig {
                publication: PublicationConfig { gate },
                rings: Vec::new(),
            },
            gates,
            SubjectProviderSet::default(),
            None,
        ))
}

async fn ringed(f: &Fixture, rings: &[(&str, &str)]) -> ManagedRepository {
    let db = test_db_pool().await;
    let roles = discover_role_directory(&AuthzHookContext {
        pool: db.pool(),
        sink: Arc::new(NullAuditSink),
        services: systemprompt_test_fixtures::fixture_services_view(),
        subjects: Default::default(),
    });
    f.repository
        .clone()
        .with_publication_policy(PublicationPolicy::new(
            ManagedMarketplaceConfig {
                publication: PublicationConfig::default(),
                rings: rings
                    .iter()
                    .map(|(dimension, value)| CohortRing {
                        dimension: (*dimension).to_owned(),
                        value: (*value).to_owned(),
                    })
                    .collect(),
            },
            Vec::new(),
            SubjectProviderSet::default(),
            roles,
        ))
}

async fn next_revision(f: &Fixture, body: &str) -> ResourceRevisionId {
    let base = f
        .repository
        .get_revision(&f.owner, &f.revision)
        .await
        .expect("base manifest");
    f.repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: f.resource.clone(),
                snapshot_id: base.snapshot_id,
                parent_id: Some(f.revision.clone()),
                files: skill_files(&f.key, body),
                dependencies: std::collections::BTreeMap::new(),
                rationale: "candidate".to_owned(),
            },
        )
        .await
        .expect("candidate revision")
}

fn improvement(
    f: &Fixture,
    revision: Option<&ResourceRevisionId>,
    action: PublicationAction,
    expected_generation: i64,
    cohort: Option<(&str, &str)>,
) -> PublicationRequest {
    let mut evidence = ComparisonEvidence::default();
    evidence
        .recorded
        .insert("note".to_owned(), serde_json::json!("compared"));
    PublicationRequest {
        resource_id: f.resource.clone(),
        revision_id: revision.cloned(),
        action,
        expected_generation,
        operation_key: format!("op-{}", Uuid::new_v4().simple()),
        comparison_evidence: evidence,
        limitations: String::new(),
        cohort: cohort.map(|(dimension, value)| CohortSelector {
            dimension: dimension.to_owned(),
            value: value.to_owned(),
        }),
    }
}

async fn consumer(roles: &[&str]) -> UserId {
    let db = test_db_pool().await;
    let id = UserId::new(format!("cohort-consumer-{}", Uuid::new_v4()));
    let roles: Vec<String> = roles.iter().map(|role| (*role).to_owned()).collect();
    seed_user_row_with_roles(&db, &id, &format!("{}@cohort.invalid", id.as_str()), &roles)
        .await
        .expect("seed consumer");
    id
}

async fn resolved_revision(
    repository: &ManagedRepository,
    f: &Fixture,
    consumer: &UserId,
) -> ResourceRevisionId {
    let rows = repository
        .list_resolutions_for(&f.owner, consumer, ResourceKind::Skill)
        .await
        .expect("cohort resolution");
    let row = rows
        .into_iter()
        .find(|row| row.resource_key == f.key)
        .expect("managed row");
    let ManagedResolution::Published { revision_id, .. } = row.resolution else {
        panic!("expected a published resolution, got {:?}", row.resolution);
    };
    revision_id
}

#[tokio::test]
async fn a_required_gate_with_none_registered_refuses_an_improvement() {
    let f = fixture().await;
    publish(&f).await;
    let candidate = next_revision(&f, "# improved\n").await;
    let repository = gated(
        &f,
        PublicationGateConfig {
            required: true,
            on_adoption: false,
        },
        Vec::new(),
    );
    let refused = repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&candidate),
                PublicationAction::PublishImprovement,
                1,
                None,
            ),
        )
        .await;
    assert!(
        matches!(refused, Err(ManagedError::Unavailable)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_required_gate_also_refuses_adoption_when_adoption_is_gated() {
    let f = fixture().await;
    let repository = gated(
        &f,
        PublicationGateConfig {
            required: true,
            on_adoption: true,
        },
        Vec::new(),
    );
    let mut request = improvement(
        &f,
        Some(&f.revision),
        PublicationAction::InitialAdoption,
        0,
        None,
    );
    request.comparison_evidence = ComparisonEvidence::default();
    let refused = repository
        .review_and_publish(&f.owner, &f.owner, &request)
        .await;
    assert!(
        matches!(refused, Err(ManagedError::Unavailable)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_refusing_gate_refuses_and_records_nothing() {
    let f = fixture().await;
    publish(&f).await;
    let candidate = next_revision(&f, "# improved\n").await;
    let repository = gated(
        &f,
        PublicationGateConfig::default(),
        vec![Arc::new(FixedGate(GateVerdict::Refuse {
            reason: "regressed".to_owned(),
        }))],
    );
    let refused = repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&candidate),
                PublicationAction::PublishImprovement,
                1,
                None,
            ),
        )
        .await;
    assert!(
        matches!(&refused, Err(ManagedError::Conflict(message)) if message.contains("regressed")),
        "{refused:?}"
    );
    let head = f
        .repository
        .publication_head_generation(&f.owner, &f.resource)
        .await
        .expect("head");
    assert_eq!(head, 1);
}

#[tokio::test]
async fn an_admitting_gate_is_recorded_on_the_review() {
    let f = fixture().await;
    publish(&f).await;
    let candidate = next_revision(&f, "# improved\n").await;
    let evidence = AssetDigest::of(b"gate evidence");
    let repository = gated(
        &f,
        PublicationGateConfig {
            required: true,
            on_adoption: false,
        },
        vec![Arc::new(FixedGate(GateVerdict::Admit {
            evidence_digest: evidence.clone(),
        }))],
    );
    let decision: PublicationDecision = repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&candidate),
                PublicationAction::PublishImprovement,
                1,
                None,
            ),
        )
        .await
        .expect("admitted");
    let db = test_db_pool().await;
    let (gate_id, digest): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT gate_id, gate_evidence_digest FROM managed_publication_reviews WHERE id=$1",
    )
    .bind(decision.review_id.as_str())
    .fetch_one(db.pool().as_ref())
    .await
    .expect("review row");
    assert_eq!(gate_id.as_deref(), Some("fixed"));
    assert_eq!(digest.as_deref(), Some(evidence.as_str()));
}

#[tokio::test]
async fn the_first_matching_ring_with_a_selection_decides() {
    let f = fixture().await;
    publish(&f).await;
    let alpha = next_revision(&f, "# alpha\n").await;
    let beta = next_revision(&f, "# beta\n").await;
    let repository = ringed(&f, &[("role", "beta"), ("role", "alpha")]).await;
    repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&alpha),
                PublicationAction::PublishImprovement,
                1,
                Some(("role", "alpha")),
            ),
        )
        .await
        .expect("alpha cohort");
    repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&beta),
                PublicationAction::PublishImprovement,
                2,
                Some(("role", "beta")),
            ),
        )
        .await
        .expect("beta cohort");

    let both = consumer(&["alpha", "beta"]).await;
    let only_alpha = consumer(&["alpha"]).await;
    let neither = consumer(&["user"]).await;
    assert_eq!(resolved_revision(&repository, &f, &both).await, beta);
    assert_eq!(resolved_revision(&repository, &f, &only_alpha).await, alpha);
    assert_eq!(
        resolved_revision(&repository, &f, &neither).await,
        f.revision
    );

    repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                None,
                PublicationAction::Withdraw,
                3,
                Some(("role", "beta")),
            ),
        )
        .await
        .expect("clear beta cohort");
    assert_eq!(resolved_revision(&repository, &f, &both).await, alpha);
}

#[tokio::test]
async fn a_cohort_outside_the_configured_rings_is_refused() {
    let f = fixture().await;
    publish(&f).await;
    let candidate = next_revision(&f, "# improved\n").await;
    let repository = ringed(&f, &[("role", "beta")]).await;
    let refused = repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&candidate),
                PublicationAction::PublishImprovement,
                1,
                Some(("role", "gamma")),
            ),
        )
        .await;
    assert!(
        matches!(refused, Err(ManagedError::Invalid(_))),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_ring_on_an_unregistered_dimension_fails_resolution() {
    let f = fixture().await;
    publish(&f).await;
    let repository = ringed(&f, &[("department", "finance")]).await;
    let reader = consumer(&["user"]).await;
    let resolution = repository
        .list_resolutions_for(&f.owner, &reader, ResourceKind::Skill)
        .await;
    assert!(
        matches!(resolution, Err(ManagedError::Invalid(_))),
        "{resolution:?}"
    );
}

#[tokio::test]
async fn the_catalog_stamp_moves_with_a_cohort_generation() {
    let f = fixture().await;
    publish(&f).await;
    let candidate = next_revision(&f, "# beta\n").await;
    let repository = ringed(&f, &[("role", "beta")]).await;
    let reader = consumer(&["beta"]).await;
    let before = repository
        .managed_catalog_stamp(&f.owner, &reader)
        .await
        .expect("stamp");
    repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &improvement(
                &f,
                Some(&candidate),
                PublicationAction::PublishImprovement,
                1,
                Some(("role", "beta")),
            ),
        )
        .await
        .expect("beta cohort");
    let after = repository
        .managed_catalog_stamp(&f.owner, &reader)
        .await
        .expect("stamp");
    assert_ne!(before, after);
}
