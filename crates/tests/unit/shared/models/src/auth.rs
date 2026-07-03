//! Unit tests for authentication models
//!
//! Tests cover:
//! - BaseRoles constants and factory methods
//! - BaseRole struct
//! - Role permissions

use systemprompt_models::BaseRoles;

#[test]
fn test_base_roles_anonymous_constant() {
    assert_eq!(BaseRoles::ANONYMOUS, "anonymous");
}

#[test]
fn test_base_roles_user_constant() {
    assert_eq!(BaseRoles::USER, "user");
}

#[test]
fn test_base_roles_admin_constant() {
    assert_eq!(BaseRoles::ADMIN, "admin");
}

#[test]
fn test_base_roles_available_roles() {
    let roles = BaseRoles::available_roles();
    assert_eq!(roles.len(), 2);
    assert!(roles.contains(&"user"));
    assert!(roles.contains(&"admin"));
}

#[test]
fn test_base_roles_available_roles_excludes_anonymous() {
    let roles = BaseRoles::available_roles();
    assert!(!roles.contains(&"anonymous"));
}

#[test]
fn test_base_roles_anonymous() {
    let role = BaseRoles::anonymous();

    assert_eq!(role.name, "anonymous");
    assert_eq!(role.display_name, "Anonymous");
    assert!(role.description.contains("Unauthenticated"));
}

#[test]
fn test_base_roles_anonymous_has_users_read_permission() {
    let role = BaseRoles::anonymous();

    assert!(role.permissions.contains("users.read"));
}

#[test]
fn test_base_roles_admin() {
    let role = BaseRoles::admin();

    assert_eq!(role.name, "admin");
    assert_eq!(role.display_name, "Administrator");
    assert!(role.description.contains("administrator"));
}

#[test]
fn test_base_roles_admin_has_empty_permissions() {
    let role = BaseRoles::admin();

    assert!(role.permissions.is_empty());
}

#[test]
fn test_base_roles_all() {
    let roles = BaseRoles::all();

    assert_eq!(roles.len(), 2);

    let names: Vec<&str> = roles.iter().map(|r| r.name).collect();
    assert!(names.contains(&"anonymous"));
    assert!(names.contains(&"admin"));
}

#[test]
fn test_base_roles_is_admin_permission_wildcard() {
    assert!(BaseRoles::is_admin_permission_wildcard());
}

#[test]
fn test_base_role_name_field() {
    let role = BaseRoles::anonymous();
    assert_eq!(role.name, "anonymous");
}

#[test]
fn test_base_role_display_name_field() {
    let role = BaseRoles::admin();
    assert_eq!(role.display_name, "Administrator");
}

#[test]
fn test_base_role_description_field() {
    let role = BaseRoles::anonymous();
    assert_eq!(
        role.description,
        "Unauthenticated user with minimal permissions"
    );
}

#[test]
fn test_base_role_permissions_field() {
    let role = BaseRoles::anonymous();
    assert_eq!(role.permissions.len(), 1);
    assert!(role.permissions.contains("users.read"));
}

#[test]
fn test_base_role_clone() {
    let role = BaseRoles::anonymous();
    let cloned = role.clone();

    assert_eq!(role.name, cloned.name);
    assert_eq!(role.display_name, cloned.display_name);
    assert_eq!(role.description, cloned.description);
    assert_eq!(role.permissions, cloned.permissions);
}

#[test]
fn test_base_role_debug() {
    let role = BaseRoles::admin();
    let debug_str = format!("{:?}", role);

    assert!(debug_str.contains("BaseRole"));
    assert!(debug_str.contains("admin"));
}

#[test]
fn test_anonymous_permissions_contains_users_read() {
    let role = BaseRoles::anonymous();
    assert!(role.permissions.contains("users.read"));
}

#[test]
fn test_anonymous_permissions_size() {
    let role = BaseRoles::anonymous();
    assert_eq!(role.permissions.len(), 1);
}

#[test]
fn test_admin_permissions_is_empty() {
    let role = BaseRoles::admin();
    assert!(role.permissions.is_empty());
}

#[test]
fn test_different_roles_have_different_names() {
    let anonymous = BaseRoles::anonymous();
    let admin = BaseRoles::admin();

    assert_ne!(anonymous.name, admin.name);
}

#[test]
fn test_different_roles_have_different_display_names() {
    let anonymous = BaseRoles::anonymous();
    let admin = BaseRoles::admin();

    assert_ne!(anonymous.display_name, admin.display_name);
}

#[test]
fn test_different_roles_have_different_permissions() {
    let anonymous = BaseRoles::anonymous();
    let admin = BaseRoles::admin();

    assert_ne!(anonymous.permissions, admin.permissions);
}

#[test]
fn test_available_roles_is_static() {
    let roles: &'static [&'static str] = BaseRoles::available_roles();
    assert_eq!(roles.len(), 2);
    assert!(roles.contains(&BaseRoles::USER));
    assert!(roles.contains(&BaseRoles::ADMIN));
}

mod act_claim {
    use systemprompt_models::auth::ActClaim;

    fn nested(sub: &str, inner: Option<ActClaim>) -> ActClaim {
        ActClaim {
            iss: format!("iss-{sub}"),
            sub: sub.to_string(),
            act: Box::new(inner),
        }
    }

    #[test]
    fn serde_round_trip_preserves_chain() {
        let chain = nested("outer", Some(nested("middle", Some(nested("inner", None)))));
        let json = serde_json::to_string(&chain).expect("serialize");
        let parsed: ActClaim = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, chain);
    }

    #[test]
    fn act_field_omitted_when_none() {
        let leaf = nested("only", None);
        let json = serde_json::to_value(&leaf).expect("serialize");
        assert!(json.get("act").is_none(), "act should be skipped when None");
    }

    #[test]
    fn flatten_three_level_chain_returns_outermost_first() {
        let chain = nested("outer", Some(nested("middle", Some(nested("inner", None)))));
        let flat = chain.flatten_to_chain();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].user_id.as_str(), "outer");
        assert_eq!(flat[1].user_id.as_str(), "middle");
        assert_eq!(flat[2].user_id.as_str(), "inner");
    }

    #[test]
    fn flatten_single_link_chain() {
        let leaf = nested("solo", None);
        let flat = leaf.flatten_to_chain();
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].user_id.as_str(), "solo");
    }
}

mod user_type_from_permissions {
    use systemprompt_models::auth::{Permission, UserType};

    #[test]
    fn admin_wins_over_lower_scopes() {
        let perms = [Permission::User, Permission::Admin, Permission::Service];
        assert_eq!(UserType::from_permissions(&perms), UserType::Admin);
    }

    #[test]
    fn precedence_is_privilege_descending() {
        assert_eq!(
            UserType::from_permissions(&[Permission::User]),
            UserType::User
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::A2a]),
            UserType::A2a
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::Mcp]),
            UserType::Mcp
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::Service]),
            UserType::Service
        );
    }

    #[test]
    fn hook_scopes_resolve_to_service_not_anon() {
        assert_eq!(
            UserType::from_permissions(&[Permission::HookGovern]),
            UserType::Service
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::HookTrack]),
            UserType::Service
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::HookGovern, Permission::HookTrack]),
            UserType::Service
        );
    }

    #[test]
    fn empty_or_anonymous_only_is_anon() {
        assert_eq!(UserType::from_permissions(&[]), UserType::Anon);
        assert_eq!(
            UserType::from_permissions(&[Permission::Anonymous]),
            UserType::Anon
        );
    }
}
