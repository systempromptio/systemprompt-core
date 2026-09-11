//! Unit tests for UserRole enum

use systemprompt_users::UserRole;

#[test]
fn as_str_returns_admin() {
    assert_eq!(UserRole::Admin.as_str(), "admin");
}

#[test]
fn as_str_returns_user() {
    assert_eq!(UserRole::User.as_str(), "user");
}

#[test]
fn as_str_returns_anonymous() {
    assert_eq!(UserRole::Anonymous.as_str(), "anonymous");
}
