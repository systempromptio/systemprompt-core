use std::str::FromStr;
use systemprompt_models::auth::{parse_permissions, permissions_to_string, Permission};

#[test]
fn test_permission_hierarchy() {
    assert!(Permission::Admin.implies(&Permission::User));
    assert!(Permission::Admin.implies(&Permission::Anonymous));
    assert!(Permission::User.implies(&Permission::Anonymous));
    assert!(!Permission::User.implies(&Permission::Admin));
    assert!(!Permission::Anonymous.implies(&Permission::User));
}

#[test]
fn test_permission_classification() {
    assert!(Permission::Admin.is_user_role());
    assert!(Permission::User.is_user_role());
    assert!(Permission::Anonymous.is_user_role());
    assert!(!Permission::A2a.is_user_role());

    assert!(Permission::A2a.is_service_scope());
    assert!(Permission::Mcp.is_service_scope());
    assert!(Permission::Service.is_service_scope());
    assert!(!Permission::Admin.is_service_scope());
}

#[test]
fn test_permission_serialization() {
    assert_eq!(Permission::Admin.as_str(), "admin");
    assert_eq!(Permission::Anonymous.as_str(), "anonymous");

    assert_eq!(Permission::from_str("admin").unwrap(), Permission::Admin);
    assert_eq!(
        Permission::from_str("anonymous").unwrap(),
        Permission::Anonymous
    );
    assert!(Permission::from_str("anon").is_err());
}

#[test]
fn test_permissions_string_conversion() {
    let permissions = vec![Permission::Admin, Permission::User, Permission::A2a];
    let s = permissions_to_string(&permissions);
    assert_eq!(s, "admin user a2a");

    let parsed = parse_permissions(&s).unwrap();
    assert_eq!(parsed, permissions);
}
