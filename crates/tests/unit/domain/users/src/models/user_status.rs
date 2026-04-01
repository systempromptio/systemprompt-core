//! Unit tests for UserStatus enum

use systemprompt_users::UserStatus;

#[test]
fn as_str_returns_active() {
    assert_eq!(UserStatus::Active.as_str(), "active");
}

#[test]
fn as_str_returns_inactive() {
    assert_eq!(UserStatus::Inactive.as_str(), "inactive");
}

#[test]
fn as_str_returns_suspended() {
    assert_eq!(UserStatus::Suspended.as_str(), "suspended");
}

#[test]
fn as_str_returns_pending() {
    assert_eq!(UserStatus::Pending.as_str(), "pending");
}

#[test]
fn as_str_returns_deleted() {
    assert_eq!(UserStatus::Deleted.as_str(), "deleted");
}

#[test]
fn as_str_returns_temporary() {
    assert_eq!(UserStatus::Temporary.as_str(), "temporary");
}

#[test]
fn user_status_clone() {
    let status = UserStatus::Suspended;
    let cloned = status;
    assert_eq!(status, cloned);
}

#[test]
fn user_status_debug() {
    let debug_str = format!("{:?}", UserStatus::Active);
    assert!(debug_str.contains("Active"));
}

#[test]
fn user_status_serializes_to_snake_case() {
    let json = serde_json::to_string(&UserStatus::Active).unwrap();
    assert_eq!(json, "\"active\"");
    let json = serde_json::to_string(&UserStatus::Inactive).unwrap();
    assert_eq!(json, "\"inactive\"");
}
