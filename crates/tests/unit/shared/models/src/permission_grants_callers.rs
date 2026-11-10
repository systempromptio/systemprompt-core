use systemprompt_identifiers::UserId;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::mcp::toolset::ToolAccess;

fn user(perms: Vec<Permission>) -> AuthenticatedUser {
    AuthenticatedUser::new(
        UserId::generate(),
        "alice".to_owned(),
        "alice@example.com".to_owned(),
        perms,
    )
}

fn scoped(scopes: Vec<Permission>) -> ToolAccess {
    ToolAccess {
        scopes,
        ..ToolAccess::default()
    }
}

#[test]
fn a_user_token_does_not_hold_service_or_hook_scopes() {
    let u = user(vec![Permission::User]);
    for scope in [
        Permission::Service,
        Permission::A2a,
        Permission::Mcp,
        Permission::HookGovern,
        Permission::HookTrack,
        Permission::Admin,
    ] {
        assert!(!u.has_permission(scope), "user token held {scope}");
    }
    assert!(u.has_permission(Permission::User));
    assert!(u.has_permission(Permission::Anonymous));
}

#[test]
fn a_toolset_requiring_a_hook_scope_refuses_a_user_token() {
    let access = scoped(vec![Permission::HookGovern]);
    assert!(!access.permits(Some(&user(vec![Permission::User])), &[]));
    assert!(access.permits(Some(&user(vec![Permission::HookGovern])), &[]));
}

#[test]
fn toolset_scopes_used_downstream_still_admit_their_callers() {
    let admin_only = scoped(vec![Permission::Admin]);
    assert!(admin_only.permits(Some(&user(vec![Permission::Admin, Permission::User])), &[]));
    assert!(!admin_only.permits(Some(&user(vec![Permission::User])), &[]));

    let user_tools = scoped(vec![Permission::User]);
    assert!(user_tools.permits(Some(&user(vec![Permission::User])), &[]));
    assert!(user_tools.permits(Some(&user(vec![Permission::Admin])), &[]));
    assert!(user_tools.permits(Some(&user(vec![Permission::A2a, Permission::User])), &[]));
    assert!(!user_tools.permits(
        Some(&user(vec![Permission::HookGovern, Permission::HookTrack])),
        &[]
    ));
}

#[test]
fn every_token_satisfies_an_anonymous_requirement() {
    let public = scoped(vec![Permission::Anonymous]);
    for held in [
        Permission::Anonymous,
        Permission::User,
        Permission::Service,
        Permission::A2a,
        Permission::Mcp,
        Permission::HookGovern,
    ] {
        assert!(
            public.permits(Some(&user(vec![held])), &[]),
            "{held} refused"
        );
    }
}
