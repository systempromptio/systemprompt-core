//! Scope checks on the MCP RBAC path: the decision that stops an
//! under-privileged but verified caller from calling tools it was never
//! granted.

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::middleware::rbac::scopes::validate_scopes_for_permissions;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::deployment::OAuthRequirement;

fn requirement(audience: JwtAudience, scopes: Vec<Permission>) -> OAuthRequirement {
    OAuthRequirement {
        required: true,
        scopes,
        audience,
        client_id: None,
        ema: false,
        service_only: false,
    }
}

// Why: the check is `grants`, not equality.
// Admin must satisfy a User requirement or every privileged caller is locked
// out of ordinary tools.
#[test]
fn a_higher_permission_satisfies_a_lower_requirement() {
    validate_scopes_for_permissions(
        &McpServerId::new("srv"),
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
        &McpServerId::new("srv"),
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
        &McpServerId::new("srv"),
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
        &McpServerId::new("srv"),
        &[Permission::Admin],
        &requirement(JwtAudience::Mcp, Vec::new()),
    )
    .expect_err("an empty requirement list cannot be satisfied by any permission");
}

#[test]
fn a_caller_holding_no_permissions_is_refused() {
    validate_scopes_for_permissions(
        &McpServerId::new("srv"),
        &[],
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect_err("a caller with no permissions must not pass a scope check");
}

#[test]
fn a_user_token_does_not_satisfy_an_mcp_scope_requirement() {
    validate_scopes_for_permissions(
        &McpServerId::new("srv"),
        &[Permission::User],
        &requirement(JwtAudience::Mcp, vec![Permission::Mcp]),
    )
    .expect_err("user scope is not the mcp scope");
}

#[test]
fn a_hook_token_does_not_satisfy_a_user_requirement() {
    validate_scopes_for_permissions(
        &McpServerId::new("srv"),
        &[Permission::HookGovern, Permission::HookTrack],
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect_err("hook scopes are not a user session");
}

#[test]
fn the_messaging_token_satisfies_a_user_requirement() {
    validate_scopes_for_permissions(
        &McpServerId::new("srv"),
        &[Permission::A2a, Permission::User],
        &requirement(JwtAudience::Mcp, vec![Permission::User]),
    )
    .expect("messaging tokens carry the sender's user scope");
}

#[test]
fn a_service_token_satisfies_an_anonymous_requirement() {
    validate_scopes_for_permissions(
        &McpServerId::new("srv"),
        &[Permission::Service],
        &requirement(JwtAudience::Mcp, vec![Permission::Anonymous]),
    )
    .expect("an anonymous requirement admits any token");
}
