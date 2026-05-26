use std::str::FromStr;

use systemprompt_models::auth::{Permission, UserType, parse_permissions, permissions_to_string};

#[test]
fn permission_round_trips_all_variants() {
    for v in [
        Permission::Admin,
        Permission::User,
        Permission::Anonymous,
        Permission::A2a,
        Permission::Mcp,
        Permission::Service,
        Permission::HookGovern,
        Permission::HookTrack,
    ] {
        let s = v.as_str();
        assert_eq!(Permission::from_str(s).unwrap(), v);
        assert_eq!(v.to_string(), s);
    }
}

#[test]
fn permission_from_str_rejects_unknown() {
    assert!(Permission::from_str("bogus").is_err());
}

#[test]
fn permission_is_valid_role_recognises_known_strings() {
    for s in [
        "admin", "user", "anonymous", "a2a", "mcp", "service", "hook:govern", "hook:track",
    ] {
        assert!(Permission::is_valid_role(s), "{s}");
    }
    assert!(!Permission::is_valid_role("ADMIN"));
    assert!(!Permission::is_valid_role("bogus"));
}

#[test]
fn permission_validate_roles_passes_for_all_valid() {
    let roles = vec!["admin".to_owned(), "user".to_owned()];
    assert!(Permission::validate_roles(&roles).is_ok());
}

#[test]
fn permission_validate_roles_returns_invalid_subset() {
    let roles = vec!["admin".to_owned(), "garbage".to_owned(), "bogus".to_owned()];
    let invalid = Permission::validate_roles(&roles).unwrap_err();
    assert_eq!(invalid.len(), 2);
    assert!(invalid.contains(&"garbage".to_owned()));
    assert!(invalid.contains(&"bogus".to_owned()));
}

#[test]
fn permission_as_user_type_maps_all_variants() {
    assert_eq!(Permission::Admin.as_user_type(), UserType::Admin);
    assert_eq!(Permission::User.as_user_type(), UserType::User);
    assert_eq!(Permission::A2a.as_user_type(), UserType::A2a);
    assert_eq!(Permission::Mcp.as_user_type(), UserType::Mcp);
    assert_eq!(Permission::Service.as_user_type(), UserType::Service);
    assert_eq!(Permission::HookGovern.as_user_type(), UserType::Service);
    assert_eq!(Permission::HookTrack.as_user_type(), UserType::Service);
    assert_eq!(Permission::Anonymous.as_user_type(), UserType::Anon);
}

#[test]
fn permission_from_user_type_maps_all_variants() {
    assert_eq!(Permission::from_user_type(UserType::Admin), Permission::Admin);
    assert_eq!(Permission::from_user_type(UserType::User), Permission::User);
    assert_eq!(Permission::from_user_type(UserType::A2a), Permission::A2a);
    assert_eq!(Permission::from_user_type(UserType::Mcp), Permission::Mcp);
    assert_eq!(Permission::from_user_type(UserType::Service), Permission::Service);
    assert_eq!(Permission::from_user_type(UserType::Anon), Permission::Anonymous);
    assert_eq!(
        Permission::from_user_type(UserType::Unknown),
        Permission::Anonymous
    );
}

#[test]
fn permission_classifier_methods() {
    assert!(Permission::Admin.is_user_role());
    assert!(Permission::User.is_user_role());
    assert!(Permission::Anonymous.is_user_role());
    assert!(!Permission::Service.is_user_role());
    assert!(!Permission::Mcp.is_user_role());

    assert!(Permission::A2a.is_service_scope());
    assert!(Permission::Mcp.is_service_scope());
    assert!(Permission::Service.is_service_scope());
    assert!(Permission::HookGovern.is_service_scope());
    assert!(Permission::HookTrack.is_service_scope());
    assert!(!Permission::Admin.is_service_scope());

    assert!(Permission::HookGovern.is_hook_scope());
    assert!(Permission::HookTrack.is_hook_scope());
    assert!(!Permission::Service.is_hook_scope());
}

#[test]
fn permission_hierarchy_level_orders_correctly() {
    assert!(Permission::Admin.hierarchy_level() > Permission::User.hierarchy_level());
    assert!(Permission::User.hierarchy_level() > Permission::Anonymous.hierarchy_level());
    assert!(Permission::Service.hierarchy_level() > Permission::Mcp.hierarchy_level());
}

#[test]
fn permission_implies_uses_hierarchy() {
    assert!(Permission::Admin.implies(&Permission::User));
    assert!(Permission::Admin.implies(&Permission::Anonymous));
    assert!(!Permission::User.implies(&Permission::Admin));
    assert!(Permission::User.implies(&Permission::User));
}

#[test]
fn permission_role_sets_have_expected_contents() {
    let user_perms = Permission::user_permissions();
    assert!(user_perms.contains(&Permission::Admin));
    assert!(user_perms.contains(&Permission::User));
    assert!(user_perms.contains(&Permission::Anonymous));
    assert!(!user_perms.contains(&Permission::Service));

    let service_perms = Permission::service_permissions();
    assert!(service_perms.contains(&Permission::A2a));
    assert!(service_perms.contains(&Permission::Mcp));
    assert!(service_perms.contains(&Permission::Service));
    assert!(!service_perms.contains(&Permission::Admin));
}

#[test]
fn permissions_to_string_joins_with_spaces() {
    let s = permissions_to_string(&[Permission::Admin, Permission::HookGovern]);
    assert_eq!(s, "admin hook:govern");
    assert_eq!(permissions_to_string(&[]), "");
}

#[test]
fn parse_permissions_splits_on_whitespace() {
    let parsed = parse_permissions("admin user mcp").unwrap();
    assert_eq!(
        parsed,
        vec![Permission::Admin, Permission::User, Permission::Mcp]
    );
    let parsed = parse_permissions("   admin\nuser ").unwrap();
    assert_eq!(parsed, vec![Permission::Admin, Permission::User]);
    let parsed = parse_permissions("").unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn parse_permissions_returns_error_on_unknown_token() {
    assert!(parse_permissions("admin garbage").is_err());
}

#[test]
fn permission_serde_lowercase() {
    let json = serde_json::to_string(&Permission::HookGovern).unwrap();
    assert_eq!(json, "\"hookgovern\"");
    let parsed: Permission = serde_json::from_str("\"admin\"").unwrap();
    assert_eq!(parsed, Permission::Admin);
}
