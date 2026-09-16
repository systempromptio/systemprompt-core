use systemprompt_evaluation::campaigns::comparison::{
    Outcome, PairedOutcome, collapse_repetitions, compare,
};
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_identifiers::{EvalBudgetId, ManagedResourceId, ResourceRevisionId};

fn policy(objective: OptimizationObjective) -> CampaignPolicy {
    CampaignPolicy {
        name: "paired-contract".to_owned(),
        resource_id: ManagedResourceId::generate(),
        baseline_revision_id: ResourceRevisionId::generate(),
        budget_id: EvalBudgetId::generate(),
        objective,
        minimum_quality_milli: 4000,
        minimum_pairs: 10,
        maximum_iterations: 3,
        automatic: false,
    }
}
fn pair() -> PairedOutcome {
    PairedOutcome {
        baseline: Outcome {
            quality_milli: 4200,
            tokens: 200,
            cost_microdollars: 400,
            latency_ms: 800,
            verified_success: true,
            hard_failures: 0,
            accounting_complete: true,
        },
        candidate: Outcome {
            quality_milli: 4500,
            tokens: 100,
            cost_microdollars: 200,
            latency_ms: 400,
            verified_success: true,
            hard_failures: 0,
            accounting_complete: true,
        },
    }
}

#[test]
fn stable_improvement_requires_complete_quality_and_accounting_for_every_objective() {
    for objective in [
        OptimizationObjective::Quality,
        OptimizationObjective::Tokens,
        OptimizationObjective::Cost,
        OptimizationObjective::Latency,
    ] {
        let policy = policy(objective);
        let pairs = vec![pair(); 10];
        let decision = compare(&policy, &pairs).unwrap();
        assert!(decision.eligible, "{:?}", decision.reasons);
        assert!(decision.lower_confidence_bound.unwrap() > 0.0);
        for variant in 0..2 {
            let mut incomplete = pairs.clone();
            if variant == 0 {
                incomplete[9].baseline.accounting_complete = false;
            } else {
                incomplete[9].candidate.accounting_complete = false;
            }
            let decision = compare(&policy, &incomplete).unwrap();
            assert!(!decision.eligible);
            assert!(
                decision
                    .reasons
                    .iter()
                    .any(|reason| reason == "Incomplete accounting")
            );
        }
    }
}

#[test]
fn positive_mean_and_higher_average_quality_do_not_replace_conservative_bounds() {
    for count in [2, 3, 4, 5, 6, 11, 21, 31] {
        let mut pairs = vec![pair(); count];
        for pair in &mut pairs {
            pair.baseline.tokens = 101;
            pair.candidate.tokens = 100;
        }
        pairs[count - 1].baseline.tokens = 1_000_100;
        let decision = compare(&policy(OptimizationObjective::Tokens), &pairs).unwrap();
        assert!(decision.mean_improvement.unwrap() > 0.0);
        assert!(decision.lower_confidence_bound.unwrap() < 0.0);
        assert!(
            !decision.eligible,
            "one outlier must not establish improvement for n={count}"
        );
    }
    let mut pairs = vec![pair(); 10];
    for pair in &mut pairs {
        pair.baseline.quality_milli = 4500;
        pair.candidate.quality_milli = 4501;
    }
    pairs[9].candidate.quality_milli = 4999;
    let decision = compare(&policy(OptimizationObjective::Tokens), &pairs).unwrap();
    assert!(!decision.eligible);
    assert!(
        decision
            .reasons
            .iter()
            .any(|reason| reason == "Quality non-regression is not established")
    );
}

#[test]
fn repetitions_collapse_conservatively_without_overflow_or_hiding_a_failure() {
    assert!(collapse_repetitions(&[]).is_err());
    let mut values = [pair(), pair()];
    values[0].baseline.tokens = 100;
    values[1].baseline.tokens = 101;
    values[0].candidate.tokens = 100;
    values[1].candidate.tokens = 101;
    values[0].baseline.cost_microdollars = u64::MAX;
    values[1].baseline.cost_microdollars = u64::MAX - 1;
    values[0].candidate.cost_microdollars = u64::MAX;
    values[1].candidate.cost_microdollars = u64::MAX - 1;
    values[0].baseline.latency_ms = 2;
    values[1].baseline.latency_ms = 3;
    values[0].candidate.latency_ms = 2;
    values[1].candidate.latency_ms = 3;
    values[1].baseline.quality_milli = 4600;
    values[1].candidate.quality_milli = 4000;
    values[1].candidate.verified_success = false;
    values[1].candidate.hard_failures = 2;
    values[1].baseline.accounting_complete = false;
    let collapsed = collapse_repetitions(&values).unwrap();
    assert_eq!(
        (collapsed.baseline.tokens, collapsed.candidate.tokens),
        (100, 101)
    );
    assert_eq!(
        (
            collapsed.baseline.cost_microdollars,
            collapsed.candidate.cost_microdollars
        ),
        (u64::MAX - 1, u64::MAX)
    );
    assert_eq!(
        (
            collapsed.baseline.latency_ms,
            collapsed.candidate.latency_ms
        ),
        (2, 3)
    );
    assert_eq!(
        (
            collapsed.baseline.quality_milli,
            collapsed.candidate.quality_milli
        ),
        (4600, 4000)
    );
    assert!(!collapsed.candidate.verified_success);
    assert_eq!(collapsed.candidate.hard_failures, 2);
    assert!(!collapsed.baseline.accounting_complete);
    let decision = compare(&policy(OptimizationObjective::Tokens), &[collapsed; 10]).unwrap();
    assert!(!decision.eligible);
    assert!(
        decision
            .reasons
            .iter()
            .any(|reason| reason == "Candidate violates quality or hard gates")
    );
}

#[test]
fn repeated_runs_are_not_independent_samples_and_invalid_scales_are_rejected() {
    let repetitions = collapse_repetitions(&[pair(); 100]).unwrap();
    let decision = compare(&policy(OptimizationObjective::Quality), &[repetitions]).unwrap();
    assert_eq!(decision.pairs, 1);
    assert!(decision.mean_improvement.is_none());
    assert!(decision.lower_confidence_bound.is_none());
    assert!(!decision.eligible);
    assert!(
        !compare(&policy(OptimizationObjective::Quality), &[])
            .unwrap()
            .eligible
    );
    for baseline in [true, false] {
        let mut invalid = pair();
        if baseline {
            invalid.baseline.quality_milli = 5001;
        } else {
            invalid.candidate.quality_milli = 5001;
        }
        assert!(compare(&policy(OptimizationObjective::Quality), &[invalid; 10]).is_err());
    }
    let mut invalid_policy = policy(OptimizationObjective::Quality);
    invalid_policy.minimum_pairs = 1;
    assert!(compare(&invalid_policy, &[pair(); 10]).is_err());
}
