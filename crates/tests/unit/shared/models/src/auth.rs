//! Unit tests for authentication models
//!
//! Tests cover:
//! - BaseRoles constants and factory methods
//! - BaseRole struct
//! - Role permissions

use systemprompt_models::BaseRoles;

// ============================================================================
// BaseRoles Constants Tests
// ============================================================================

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

// ============================================================================
// BaseRoles Factory Methods Tests
// ============================================================================

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

    // Admin has wildcard access, so permissions set is empty
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

// ============================================================================
// BaseRole Struct Tests
// ============================================================================

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
    assert!(!role.description.is_empty());
}

#[test]
fn test_base_role_permissions_field() {
    let role = BaseRoles::anonymous();
    assert!(!role.permissions.is_empty());
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

// ============================================================================
// Permissions Tests
// ============================================================================

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

// ============================================================================
// Role Comparison Tests
// ============================================================================

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

// ============================================================================
// Static Lifetime Tests
// ============================================================================

#[test]
fn test_base_roles_constants_are_static() {
    // These should compile because the constants are 'static
    let _: &'static str = BaseRoles::ANONYMOUS;
    let _: &'static str = BaseRoles::USER;
    let _: &'static str = BaseRoles::ADMIN;
}

#[test]
fn test_available_roles_is_static() {
    let roles: &'static [&'static str] = BaseRoles::available_roles();
    assert!(!roles.is_empty());
}

#[test]
fn test_base_role_name_is_static() {
    let role = BaseRoles::anonymous();
    let _: &'static str = role.name;
}

#[test]
fn test_base_role_display_name_is_static() {
    let role = BaseRoles::admin();
    let _: &'static str = role.display_name;
}

#[test]
fn test_base_role_description_is_static() {
    let role = BaseRoles::anonymous();
    let _: &'static str = role.description;
}
