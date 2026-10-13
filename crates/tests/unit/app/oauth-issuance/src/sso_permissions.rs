use systemprompt_models::auth::Permission;
use systemprompt_oauth_issuance::sso::permissions_for_roles;

#[test]
fn admin_role_grants_admin_and_user() {
    assert_eq!(
        permissions_for_roles(&["user".to_owned(), "admin".to_owned()]),
        vec![Permission::Admin, Permission::User]
    );
}

#[test]
fn other_roles_grant_user_only() {
    assert_eq!(
        permissions_for_roles(&["editor".to_owned()]),
        vec![Permission::User]
    );
    assert_eq!(permissions_for_roles(&[]), vec![Permission::User]);
}
