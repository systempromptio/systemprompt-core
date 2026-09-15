//! The evaluation evidence digest is canonical JSON: field order never changes
//! the bytes the attestation compares.

use systemprompt_evaluation::campaigns::comparison::ComparisonDecision;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_identifiers::{
    EvalBudgetId, EvalExperimentId, ManagedResourceId, ResourceRevisionId,
};
use systemprompt_marketplace::managed::AssetDigest;
use systemprompt_runtime::optimization::EvaluationEvidence;

fn decision(eligible: bool) -> ComparisonDecision {
    ComparisonDecision {
        eligible,
        pairs: 4,
        mean_improvement: Some(0.25),
        lower_confidence_bound: Some(0.1),
        reasons: vec![],
    }
}

#[test]
fn evidence_digest_is_canonical_json_independent_of_field_order() {
    let policy = CampaignPolicy {
        name: "jcs".to_owned(),
        resource_id: ManagedResourceId::new("resource-1"),
        baseline_revision_id: ResourceRevisionId::new("revision-1"),
        budget_id: EvalBudgetId::new("budget-1"),
        objective: OptimizationObjective::Tokens,
        minimum_quality_milli: 4000,
        minimum_pairs: 2,
        maximum_iterations: 3,
        automatic: true,
    };
    let experiment_id = EvalExperimentId::new("experiment-1");
    let development = decision(true);
    let holdout = decision(false);
    let evidence = EvaluationEvidence {
        policy: &policy,
        experiment_id: &experiment_id,
        baseline_bundle_digest: "a",
        candidate_bundle_digest: "b",
        development: &development,
        holdout: &holdout,
    };
    let digest = evidence.digest().expect("digest");

    let reordered = serde_json::json!({
        "holdout": holdout,
        "development": development,
        "candidate_bundle_digest": "b",
        "baseline_bundle_digest": "a",
        "experiment_id": "experiment-1",
        "policy": policy,
    });
    let expected = AssetDigest::of(&serde_jcs::to_vec(&reordered).expect("jcs"));
    assert_eq!(digest, expected);

    let non_canonical = AssetDigest::of(&serde_json::to_vec(&reordered).expect("json"));
    assert_ne!(
        digest, non_canonical,
        "plain serde_json bytes are order-dependent"
    );
}
