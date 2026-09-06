//! Audience and scope checks on the MCP RBAC path.
//!
//! These are the two authorisation decisions made once a token has been
//! verified. Audience confinement is what stops a token minted for one
//! surface being replayed against an MCP server; the scope check is what
//! stops an under-privileged but validly-scoped token from calling tools it
//! was never granted.

use chrono::{Duration, Utc};
use systemprompt_identifiers::{ClientId, SessionId};
use systemprompt_mcp::test_api::{validate_audience, validate_scopes_for_permissions};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_models::mcp::deployment::OAuthRequirement;

fn claims_for(audiences: Vec<JwtAudience>) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: "user_42".to_owned(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: "issuer".to_owned(),
        aud: audiences,
        jti: "jti-1".to_owned(),
        scope: vec![Permission::User],
        username: "u".to_owned(),
        email: "u@example.com".to_owned(),
        user_type: UserType::User,
        roles: Vec::new(),
        attributes: std::collections::BTreeMap::new(),
        client_id: Some(ClientId::new("c")),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(SessionId::new("s")),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    }
}

fn requirement(audience: JwtAudience, scopes: Vec<Permission>) -> OAuthRequirement {
    OAuthRequirement {
        required: true,
        scopes,
        audience,
        client_id: None,
        ema: false,
    }
}

#[test]
fn a_token_carrying_the_servers_audience_is_accepted() {
    validate_audience(
        "srv",
        &claims_for(vec![JwtAudience::Mcp]),
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect("the audience matches and must be accepted");
}

// Why: this is the confinement boundary. A token minted for the A2A surface
// replayed at an MCP server must be refused, or one audience's credential
// silently authorises another's tools.
#[test]
fn a_token_minted_for_a_different_audience_is_refused() {
    let err = validate_audience(
        "srv",
        &claims_for(vec![JwtAudience::A2a]),
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect_err("a foreign audience must not be accepted");

    let message = format!("{err:?}");
    assert!(
        message.contains("Invalid audience"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("mcp"),
        "the expected audience must be named: {message}"
    );
}

// Why: a multi-audience token is legitimate. Requiring the server's audience
// to be the *only* one would reject tokens the issuer deliberately scoped to
// several surfaces.
#[test]
fn a_token_listing_several_audiences_is_accepted_when_one_of_them_matches() {
    validate_audience(
        "srv",
        &claims_for(vec![JwtAudience::A2a, JwtAudience::Mcp]),
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect("one matching audience is enough");
}

#[test]
fn a_token_with_no_audience_at_all_is_refused() {
    validate_audience(
        "srv",
        &claims_for(Vec::new()),
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect_err("an audience-less token must not reach a server that requires one");
}

// Why: permissions are hierarchical, so the check is `implies`, not equality.
// Admin must satisfy a User requirement or every privileged caller is locked
// out of ordinary tools.
#[test]
fn a_higher_permission_satisfies_a_lower_requirement() {
    validate_scopes_for_permissions(
        "srv",
        &[Permission::Admin],
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect("admin implies user");
}

// Why: the converse is the actual guard. Anonymous must not reach a tool that
// asked for User.
#[test]
fn a_lower_permission_does_not_satisfy_a_higher_requirement() {
    let err = validate_scopes_for_permissions(
        "srv",
        &[Permission::Anonymous],
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect_err("anonymous must not satisfy a user requirement");

    assert!(
        format!("{err:?}").contains("Insufficient permissions"),
        "unexpected message: {err:?}"
    );
}

// Why: the requirement list is any-of, so holding one of several accepted
// scopes is enough.
#[test]
fn holding_one_of_several_accepted_scopes_is_enough() {
    validate_scopes_for_permissions(
        "srv",
        &[Permission::User],
        &requirement(JwtAudience::Mcp, vec![Permission::Admin, Permission::User]),
    )
    .expect("one satisfied requirement is enough");
}

// Why: a server that requires no scopes has nothing for the caller to satisfy,
// so the any-of fold is empty and must refuse rather than wave everyone
// through. This is the fail-closed direction.
#[test]
fn a_requirement_listing_no_scopes_is_not_satisfiable() {
    validate_scopes_for_permissions(
        "srv",
        &[Permission::Admin],
        &requirement(JwtAudience::Mcp, Vec::new()),
    )
    .expect_err("an empty requirement list cannot be satisfied by any permission");
}

#[test]
fn a_caller_holding_no_permissions_is_refused() {
    validate_scopes_for_permissions(
        "srv",
        &[],
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect_err("a caller with no permissions must not pass a scope check");
}
