use systemprompt_brain::jobs::{BrainUsageJob, laplace_rates};
use systemprompt_traits::Job;

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

#[test]
fn a_fact_never_injected_sits_at_the_prior() {
    let rates = laplace_rates(0, 0);
    assert!(close(rates.cited_rate, 0.5));
    assert!(close(rates.ignored_rate, 0.5));
}

#[test]
fn rates_are_laplace_smoothed_shares_of_injections() {
    let rates = laplace_rates(4, 3);
    assert!(close(rates.cited_rate, 4.0 / 6.0));
    assert!(close(rates.ignored_rate, 2.0 / 6.0));
    let never_cited = laplace_rates(8, 0);
    assert!(close(never_cited.cited_rate, 0.1));
    assert!(close(never_cited.ignored_rate, 0.9));
}

#[test]
fn citations_above_injections_are_capped() {
    let rates = laplace_rates(2, 5);
    assert!(close(rates.cited_rate, 0.75));
    assert!(close(rates.ignored_rate, 0.25));
}

#[test]
fn the_job_runs_daily_before_retention_and_analysis() {
    assert_eq!(BrainUsageJob.name(), "brain_usage");
    assert_eq!(BrainUsageJob.schedule(), "0 45 2 * * *");
    assert_eq!(
        BrainUsageJob.scope(),
        systemprompt_traits::JobScope::Cluster
    );
}
