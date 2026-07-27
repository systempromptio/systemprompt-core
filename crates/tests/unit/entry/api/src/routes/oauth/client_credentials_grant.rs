//! Scope and audience policy for the `client_credentials` grant.
//!
//! The grant has a two-tier scope rule: service-tier scopes need only the
//! client's own static grant, while user-tier roles are delegated authority and
//! require the client *and* its owner to hold them. Getting that wrong either
//! hands a machine client its owner's admin rights or breaks legitimate
//! service tokens, so the tier split, the rejection reasons, and the audience
//! narrowing are pinned here.

use systemprompt_api::routes::oauth::endpoints::token::generation::ClientCredentialsError;
use systemprompt_api::routes::oauth::endpoints::token::generation::client_credentials_test_api::{
    authorize_client_grant, resolve_audience, scope_permissions,
};
use systemprompt_models::Config;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_test_fixtures::fixture_config;

fn scopes(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_owned()).collect()
}

fn config_with_audiences(allowed: &[&str]) -> Config {
    let mut config = fixture_config("postgres://unused/unused");
    config.allowed_resource_audiences = allowed.iter().map(|s| (*s).to_owned()).collect();
    config
}

fn invalid_scope_reason(error: ClientCredentialsError) -> String {
    match error {
        ClientCredentialsError::InvalidScope(reason) => reason,
        other => panic!("expected InvalidScope, got {other:?}"),
    }
}

#[test]
fn unparseable_scope_strings_are_dropped_rather_than_failing_the_whole_set() {
    let permissions = scope_permissions(&scopes(&["admin", "not-a-scope", "mcp"]));

    assert_eq!(permissions, vec![Permission::Admin, Permission::Mcp]);
}

#[test]
fn a_service_tier_scope_needs_only_the_client_grant() {
    let granted = authorize_client_grant(
        &[Permission::Service],
        &scopes(&["service"]),
        &[Permission::User],
    )
    .expect("a service-tier scope the client holds must be granted");

    assert_eq!(granted, vec![Permission::Service]);
}

#[test]
fn a_user_tier_role_requires_the_owner_to_hold_it_too() {
    let reason = invalid_scope_reason(
        authorize_client_grant(
            &[Permission::Admin],
            &scopes(&["admin"]),
            &[Permission::User],
        )
        .expect_err("a client must not out-scope its owner"),
    );

    assert!(
        reason.contains("delegated scopes not held by owner"),
        "{reason}"
    );
    assert!(reason.contains("admin"), "{reason}");
}

#[test]
fn a_user_tier_role_is_granted_when_both_client_and_owner_hold_it() {
    let granted = authorize_client_grant(
        &[Permission::Admin],
        &scopes(&["admin"]),
        &[Permission::Admin],
    )
    .expect("client and owner both hold admin");

    assert_eq!(granted, vec![Permission::Admin]);
}

#[test]
fn a_scope_outside_the_client_grant_is_reported_as_such() {
    let reason = invalid_scope_reason(
        authorize_client_grant(
            &[Permission::Admin],
            &scopes(&["mcp"]),
            &[Permission::Admin],
        )
        .expect_err("the client was never granted admin"),
    );

    assert!(
        reason.contains("requested scopes not in client grant"),
        "{reason}"
    );
}

#[test]
fn an_empty_request_is_rejected_with_the_no_scopes_reason() {
    let reason = invalid_scope_reason(
        authorize_client_grant(&[], &scopes(&["service"]), &[Permission::Admin])
            .expect_err("a token with no scopes is not issuable"),
    );

    assert_eq!(reason, "no scopes requested");
}

#[test]
fn a_mixed_request_keeps_the_permitted_scopes_and_drops_the_rest() {
    let granted = authorize_client_grant(
        &[Permission::Admin, Permission::Mcp],
        &scopes(&["admin", "mcp"]),
        &[Permission::User],
    )
    .expect("mcp is service-tier and survives even though admin is denied");

    assert_eq!(granted, vec![Permission::Mcp]);
}

#[test]
fn granted_scopes_are_ordered_by_descending_privilege_and_deduplicated() {
    let granted = authorize_client_grant(
        &[Permission::Mcp, Permission::Admin, Permission::Admin],
        &scopes(&["admin", "mcp"]),
        &[Permission::Admin],
    )
    .expect("both scopes are held by client and owner");

    assert_eq!(granted, vec![Permission::Admin, Permission::Mcp]);
}

#[test]
fn an_absent_audience_falls_back_to_the_configured_default_set() {
    let config = config_with_audiences(&["hook"]);

    let audience =
        resolve_audience(None, &config).expect("the default audience set always resolves");

    assert_eq!(audience, config.jwt_audiences);
}

#[test]
fn an_allowed_audience_narrows_the_token_to_that_audience_alone() {
    let config = config_with_audiences(&["hook"]);

    let audience = resolve_audience(Some("hook"), &config).expect("hook is in the allowlist");

    assert_eq!(audience, vec![JwtAudience::Hook]);
}

#[test]
fn an_audience_outside_the_allowlist_is_an_invalid_audience_error() {
    let config = config_with_audiences(&["hook"]);

    let error = resolve_audience(Some("mcp"), &config)
        .expect_err("an unlisted audience must not be minted into a token");

    match error {
        ClientCredentialsError::InvalidAudience(message) => {
            assert!(message.contains("'mcp'"), "{message}");
            assert!(message.contains("not in allowed audiences"), "{message}");
        },
        other => panic!("expected InvalidAudience, got {other:?}"),
    }
}
