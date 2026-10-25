//! `describe_route_match` — the audit string that records *why* a request
//! landed on the route it did (the matched policy route, or the catalog route).
//!
//! The descriptor is what an operator reads back when a request went somewhere
//! unexpected, so it always opens with the route's id: for nine days in
//! 2026-10 every Claude Code turn on one installation was served by a route
//! nobody expected, and the audit rows could not say which. Each further
//! reason is recorded under its own tag, and a route chosen by more than one
//! reason keeps all of them.

use systemprompt_gateway::service::route_match::describe_route_match;
use systemprompt_identifiers::RouteId;
use systemprompt_manifest::services::{GatewayRoute, RouteRequirements};

fn route(requires: Option<RouteRequirements>) -> GatewayRoute {
    GatewayRoute {
        requires,
        ..GatewayRoute::new("eu-route")
    }
}

fn id() -> RouteId {
    RouteId::new("eu-route")
}

#[test]
fn a_plain_pattern_match_still_records_the_route() {
    assert_eq!(
        describe_route_match(Some(&id()), Some(&route(None)), None, None),
        "route:eu-route".to_owned(),
        "which route served a request is never left blank"
    );
}

#[test]
fn a_declarative_predicate_is_recorded_on_its_own() {
    assert_eq!(
        describe_route_match(
            Some(&id()),
            Some(&route(None)),
            Some("when:tools".to_owned()),
            None
        ),
        "route:eu-route;when:tools".to_owned()
    );
}

#[test]
fn a_selector_is_recorded_on_its_own() {
    assert_eq!(
        describe_route_match(
            Some(&id()),
            Some(&route(None)),
            None,
            Some("selector:cheap".to_owned())
        ),
        "route:eu-route;selector:cheap".to_owned()
    );
}

#[test]
fn route_governance_alone_is_enough_to_produce_a_descriptor() {
    let requires = RouteRequirements {
        european: true,
        no_retain: true,
    };

    assert_eq!(
        describe_route_match(Some(&id()), Some(&route(Some(requires))), None, None),
        "route:eu-route;requires:european,no_retain".to_owned(),
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
        describe_route_match(Some(&id()), Some(&route(Some(requires))), None, None),
        "route:eu-route".to_owned(),
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
            Some(&id()),
            Some(&route(Some(requires))),
            Some("when:tools".to_owned()),
            Some("selector:cheap".to_owned()),
        ),
        "route:eu-route;when:tools;selector:cheap;requires:european".to_owned(),
        "a selector refinement must not erase the declarative predicate that \
         chose the candidate route, nor the governance it carries"
    );
}
