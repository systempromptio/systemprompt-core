//! Unit tests for UserRole enum

use systemprompt_users::UserRole;

#[test]
fn as_str_returns_admin() { assert_eq!(UserRole::Admin.as_str(), "admin"); }

#[test]
fn as_str_returns_user() { assert_eq!(UserRole::User.as_str(), "user"); }

#[test]
fn as_str_returns_anonymous() { assert_eq!(UserRole::Anonymous.as_str(), "anonymous"); }

#[test]
fn user_role_equality() {
    assert_eq!(UserRole::Admin, UserRole::Admin);
    assert_ne!(UserRole::Admin, UserRole::User);
    assert_ne!(UserRole::User, UserRole::Anonymous);
}

#[test]
fn user_role_clone() { let role = UserRole::Admin; let cloned = role; assert_eq!(role, cloned); }

#[test]
fn user_role_copy() { let role = UserRole::User; let copied = role; assert_eq!(role, copied); }

#[test]
fn user_role_debug() {
    let debug_str = format!("{:?}", UserRole::Admin);
    assert!(debug_str.contains("Admin"));
}

#[test]
fn user_role_serializes_to_snake_case() {
    assert_eq!(serde_json::to_string(&UserRole::Admin).unwrap(), "\"admin\"");
    assert_eq!(serde_json::to_string(&UserRole::User).unwrap(), "\"user\"");
    assert_eq!(serde_json::to_string(&UserRole::Anonymous).unwrap(), "\"anonymous\"");
}

#[test]
fn user_role_deserializes_from_snake_case() {
    let role: UserRole = serde_json::from_str("\"admin\"").unwrap();
    assert_eq!(role, UserRole::Admin);
    let role: UserRole = serde_json::from_str("\"user\"").unwrap();
    assert_eq!(role, UserRole::User);
    let role: UserRole = serde_json::from_str("\"anonymous\"").unwrap();
    assert_eq!(role, UserRole::Anonymous);
}

#[test]
fn user_role_all_variants_round_trip() {
    for variant in [UserRole::Admin, UserRole::User, UserRole::Anonymous] {
        let json = serde_json::to_string(&variant).unwrap();
        let deserialized: UserRole = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, deserialized);
    }
}
