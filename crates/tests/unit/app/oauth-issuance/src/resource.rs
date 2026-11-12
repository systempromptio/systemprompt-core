//! RFC 8707 resource resolution for bound and unbound grants.

use systemprompt_manifest::Config;
use systemprompt_oauth_issuance::IssuanceError;
use systemprompt_oauth_issuance::resource::{ResourceBinding, resolve_grant_resource};
use systemprompt_test_fixtures::fixture_config;

const BOUND: &str = "https://mcp-a.test";
const FOREIGN: &str = "https://anything.test";

fn config() -> Config {
    let mut config = fixture_config("postgres://unused/unused");
    config.allowed_resource_audiences = vec![FOREIGN.to_owned()];
    config
}

fn refused(result: Result<Option<&str>, IssuanceError>) -> String {
    match result {
        Err(IssuanceError::InvalidTarget { message }) => message,
        other => panic!("expected invalid_target, got {other:?}"),
    }
}

#[test]
fn a_bound_grant_without_a_requested_resource_keeps_its_own() {
    let resolved = resolve_grant_resource(None, ResourceBinding::Bound(Some(BOUND)), &config())
        .expect("omitted resource");
    assert_eq!(resolved, Some(BOUND));
}

#[test]
fn a_bound_grant_may_name_exactly_its_resource() {
    let resolved =
        resolve_grant_resource(Some(BOUND), ResourceBinding::Bound(Some(BOUND)), &config())
            .expect("same resource");
    assert_eq!(resolved, Some(BOUND));
}

#[test]
fn a_bound_grant_refuses_another_resource_even_when_allowlisted() {
    let message = refused(resolve_grant_resource(
        Some(FOREIGN),
        ResourceBinding::Bound(Some(BOUND)),
        &config(),
    ));
    assert!(
        message.contains(FOREIGN) && message.contains(BOUND),
        "{message}"
    );
}

#[test]
fn a_grant_bound_to_no_resource_never_gains_one() {
    let message = refused(resolve_grant_resource(
        Some(FOREIGN),
        ResourceBinding::Bound(None),
        &config(),
    ));
    assert!(message.contains(FOREIGN), "{message}");
    assert_eq!(
        resolve_grant_resource(None, ResourceBinding::Bound(None), &config()).expect("none"),
        None
    );
}

#[test]
fn an_unbound_grant_is_held_to_the_allowlist() {
    assert_eq!(
        resolve_grant_resource(Some(FOREIGN), ResourceBinding::Unbound, &config())
            .expect("allowlisted"),
        Some(FOREIGN)
    );
    refused(resolve_grant_resource(
        Some(BOUND),
        ResourceBinding::Unbound,
        &config(),
    ));
}
