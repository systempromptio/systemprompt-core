//! The deployment selection decision, kept free of I/O so it can be tested as
//! a table: given each deployment's breaker state, weight and in-flight count,
//! the order a route's chain is attempted in.
//!
//! `ordered` keeps [`plan_attempts`]. `weighted` draws the first attempt by
//! weight among the healthy deployments and orders the rest by descending
//! weight (chain order breaks ties). `least_busy` puts the healthy deployment
//! with the fewest requests in flight first (chain order breaks ties) and
//! keeps the rest in chain order. Tripped deployments always come last, in
//! chain order, so a request is never left unsent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::SelectionStrategy;

use super::decision::plan_attempts;

/// One deployment as the selection decision sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeploymentState {
    pub tripped: bool,
    pub weight: u32,
    pub in_flight: u64,
}

/// The full attempt order over `states`; `draw` is a uniform random number
/// that only `weighted` consumes.
#[must_use]
pub fn plan_selection(
    strategy: SelectionStrategy,
    states: &[DeploymentState],
    draw: u64,
) -> Vec<usize> {
    let healthy: Vec<usize> = (0..states.len()).filter(|&i| !states[i].tripped).collect();
    let tripped: Vec<usize> = (0..states.len()).filter(|&i| states[i].tripped).collect();
    let mut order = match strategy {
        SelectionStrategy::Ordered => {
            return plan_attempts(&states.iter().map(|s| s.tripped).collect::<Vec<_>>());
        },
        SelectionStrategy::Weighted => weighted(states, &healthy, draw),
        SelectionStrategy::LeastBusy => least_busy(states, &healthy),
    };
    order.extend(tripped);
    order
}

fn weighted(states: &[DeploymentState], healthy: &[usize], draw: u64) -> Vec<usize> {
    let mut rest: Vec<usize> = healthy.to_vec();
    rest.sort_by(|a, b| states[*b].weight.cmp(&states[*a].weight));
    let total: u64 = healthy.iter().map(|&i| u64::from(states[i].weight.max(1))).sum();
    let Some(&default_first) = healthy.first() else {
        return rest;
    };
    let mut point = draw % total.max(1);
    let mut first = default_first;
    for &i in healthy {
        let weight = u64::from(states[i].weight.max(1));
        if point < weight {
            first = i;
            break;
        }
        point -= weight;
    }
    rest.retain(|&i| i != first);
    let mut order = vec![first];
    order.extend(rest);
    order
}

fn least_busy(states: &[DeploymentState], healthy: &[usize]) -> Vec<usize> {
    let Some(&first) = healthy.iter().min_by_key(|&&i| states[i].in_flight) else {
        return Vec::new();
    };
    let mut order = vec![first];
    order.extend(healthy.iter().copied().filter(|&i| i != first));
    order
}
