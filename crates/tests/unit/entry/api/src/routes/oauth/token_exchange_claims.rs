//! RFC 8707 `resource` and `audience` narrowing, and the JWKS host allowlist.
//!
//! `validate_resource` and `resolve_audience` are the two places where a
//! token-exchange caller can ask for a token aimed at something other than this
//! deployment; both are gated on `allowed_resource_audiences`. The allowlist
//! that bounds JWKS fetches is derived from the same config, so it is pinned
//! here too.

use systemprompt_api::routes::oauth::endpoints::token::generation::test_api::{
    jwks_host_allowlist, resolve_audience, validate_resource,
};
use systemprompt_models::Config;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_test_fixtures::fixture_config;

fn config_with_audiences(allowed: &[&str]) -> Config {
    let mut config = fixture_config("postgres://unused/unused");
    config.allowed_resource_audiences = allowed.iter().map(|s| (*s).to_owned()).collect();
    config
}

fn issuer(jwks_uri: &str) -> TrustedIssuer {
    TrustedIssuer {
        issuer: "https://idp.test".to_owned(),
        jwks_uri: jwks_uri.to_owned(),
        audience: "systemprompt".to_owned(),
        typ_allowlist: vec![],
        allowed_client_ids: vec![],
        can_issue_id_jag: false,
    }
}

#[test]
fn resource_absent_is_accepted_unchanged() {
    let resolved = validate_resource(None, &config_with_audiences(&[]))
        .expect("no resource requested is always valid");

    assert_eq!(resolved, None);
}

#[test]
fn an_allowed_resource_passes_through() {
    let config = config_with_audiences(&["https://api.test", "https://other.test"]);

    let resolved =
        validate_resource(Some("https://api.test"), &config).expect("listed resource is allowed");

    assert_eq!(resolved, Some("https://api.test"));
}

#[test]
fn an_unlisted_resource_is_rejected_as_invalid_target() {
    let config = config_with_audiences(&["https://api.test"]);

    let message = validate_resource(Some("https://evil.test"), &config)
        .expect_err("an unlisted resource must be rejected")
        .to_string();

    assert!(
        message.contains("https://evil.test") && message.contains("allowed_resource_audiences"),
        "{message}"
    );
}

#[test]
fn any_resource_is_rejected_when_the_allowlist_is_empty() {
    let message = validate_resource(Some("https://api.test"), &config_with_audiences(&[]))
        .expect_err("an empty allowlist admits nothing")
        .to_string();

    assert!(
        message.contains("not in allowed_resource_audiences"),
        "{message}"
    );
}

#[test]
fn audience_defaults_to_the_configured_jwt_audiences() {
    let config = config_with_audiences(&["https://api.test"]);

    let resolved =
        resolve_audience(None, &config).expect("the default audience set always resolves");

    assert_eq!(resolved, config.jwt_audiences);
}

#[test]
fn an_unlisted_audience_is_rejected_as_invalid_target() {
    let config = config_with_audiences(&["https://api.test"]);

    let message = resolve_audience(Some("https://evil.test"), &config)
        .expect_err("an unlisted audience must be rejected")
        .to_string();

    assert!(
        message.contains("audience 'https://evil.test'")
            && message.contains("allowed_resource_audiences"),
        "{message}"
    );
}

#[test]
fn an_allowed_audience_narrows_the_token_to_exactly_that_audience() {
    let config = config_with_audiences(&["api"]);

    let resolved = resolve_audience(Some("api"), &config).expect("listed audience is allowed");

    assert_eq!(resolved, vec![JwtAudience::Api]);
    assert!(
        resolved.len() < config.jwt_audiences.len(),
        "an explicit audience must narrow, not widen, the default set"
    );
}

#[test]
fn jwks_allowlist_collects_the_host_of_every_trusted_issuer() {
    let trusted = vec![
        issuer("https://idp-one.test/.well-known/jwks.json"),
        issuer("https://idp-two.test:8443/jwks"),
    ];

    let hosts = jwks_host_allowlist(&trusted);

    assert_eq!(hosts, vec!["idp-one.test", "idp-two.test"]);
}

#[test]
fn jwks_allowlist_drops_entries_with_an_unparseable_uri() {
    let trusted = vec![issuer("not a url"), issuer("https://idp.test/jwks")];

    let hosts = jwks_host_allowlist(&trusted);

    assert_eq!(hosts, vec!["idp.test"]);
}

#[test]
fn jwks_allowlist_is_empty_when_no_issuer_is_trusted() {
    assert!(jwks_host_allowlist(&[]).is_empty());
}
