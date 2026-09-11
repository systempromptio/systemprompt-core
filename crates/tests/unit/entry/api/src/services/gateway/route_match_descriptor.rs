//! `describe_route_match` — the audit string that records *why* a request
//! landed on the route it did.
//!
//! The descriptor is what an operator reads back when a request went somewhere
//! unexpected, so an empty descriptor has to mean "nothing selected this route
//! but its model pattern" rather than "we forgot to record it". Each of the
//! three reasons is recorded under its own tag, and a route chosen by more
//! than one reason keeps all of them.

use systemprompt_api::services::gateway::service::resolve::describe_route_match;
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::services::{GatewayRoute, RouteRequirements};

fn route(requires: Option<RouteRequirements>) -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new("eu-route"),
        model_pattern: "model-*".to_owned(),
        provider: ProviderId::new("acme"),
        upstream_model: None,
        extra_headers: Default::default(),
        pricing: None,
        when: None,
        requires,
    }
}

#[test]
fn a_plain_pattern_match_records_no_reason_at_all() {
    assert_eq!(
        describe_route_match(&route(None), None, None),
        None,
        "a route matched only by its model pattern has nothing to explain"
    );
}

#[test]
fn a_declarative_predicate_is_recorded_on_its_own() {
    assert_eq!(
        describe_route_match(&route(None), Some("when:tools".to_owned()), None),
        Some("when:tools".to_owned())
    );
}

#[test]
fn a_selector_is_recorded_on_its_own() {
    assert_eq!(
        describe_route_match(&route(None), None, Some("selector:cheap".to_owned())),
        Some("selector:cheap".to_owned())
    );
}

#[test]
fn route_governance_alone_is_enough_to_produce_a_descriptor() {
    let requires = RouteRequirements {
        european: true,
        no_retain: true,
    };

    assert_eq!(
        describe_route_match(&route(Some(requires)), None, None),
        Some("requires:european,no_retain".to_owned()),
        "a route's compliance promises belong in the audit trail even when \
         nothing else steered the request to it"
    );
}

#[test]
fn a_requirements_block_that_declares_nothing_is_not_a_reason() {
    let requires = RouteRequirements {
        european: false,
        no_retain: false,
    };

    assert_eq!(
        describe_route_match(&route(Some(requires)), None, None),
        None,
        "an empty `requires:` block must not be reported as a governance match"
    );
}

#[test]
fn every_reason_is_kept_and_ordered_predicate_selector_governance() {
    let requires = RouteRequirements {
        european: true,
        no_retain: false,
    };

    assert_eq!(
        describe_route_match(
            &route(Some(requires)),
            Some("when:tools".to_owned()),
            Some("selector:cheap".to_owned()),
        ),
        Some("when:tools;selector:cheap;requires:european".to_owned()),
        "a selector refinement must not erase the declarative predicate that \
         chose the candidate route, nor the governance it carries"
    );
}
