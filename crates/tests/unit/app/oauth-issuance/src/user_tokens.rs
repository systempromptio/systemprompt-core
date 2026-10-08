use systemprompt_models::auth::Permission;
use systemprompt_oauth_issuance::resolve_user_permissions;

#[test]
fn test_resolve_user_permissions_user_expands_to_user_roles() {
    let requested = vec![Permission::User];
    let user_perms = vec![Permission::Admin, Permission::User, Permission::A2a];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    assert!(result.contains(&Permission::Admin));
    assert!(result.contains(&Permission::User));
    assert!(!result.contains(&Permission::A2a));
}

#[test]
fn test_resolve_user_permissions_specific_permission_matched() {
    let requested = vec![Permission::Admin];
    let user_perms = vec![Permission::Admin, Permission::User];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    assert!(result.contains(&Permission::Admin));
    assert!(!result.contains(&Permission::User));
}

#[test]
fn test_resolve_user_permissions_unmatched_permission_excluded() {
    let requested = vec![Permission::Admin];
    let user_perms = vec![Permission::User, Permission::Anonymous];
    let result = resolve_user_permissions(&requested, &user_perms);
    assert!(result.is_err());
}

#[test]
fn test_resolve_user_permissions_empty_requested_returns_error() {
    let requested: Vec<Permission> = vec![];
    let user_perms = vec![Permission::Admin, Permission::User];
    let result = resolve_user_permissions(&requested, &user_perms);
    assert!(result.is_err());
}

#[test]
fn test_resolve_user_permissions_deduplicates() {
    let requested = vec![Permission::User, Permission::Admin];
    let user_perms = vec![Permission::Admin, Permission::User];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    let admin_count = result.iter().filter(|p| **p == Permission::Admin).count();
    assert_eq!(admin_count, 1);
}

#[test]
fn test_resolve_user_permissions_sorted_by_hierarchy() {
    let requested = vec![Permission::User, Permission::Admin];
    let user_perms = vec![Permission::Admin, Permission::User, Permission::Anonymous];
    let result = resolve_user_permissions(&requested, &user_perms).unwrap();
    assert!(result[0].hierarchy_level() >= result[result.len() - 1].hierarchy_level());
}
