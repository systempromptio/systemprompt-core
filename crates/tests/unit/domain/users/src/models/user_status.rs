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
fn user_status_equality() {
    assert_eq!(UserStatus::Active, UserStatus::Active);
    assert_ne!(UserStatus::Active, UserStatus::Inactive);
}

#[test]
fn user_status_clone() {
    let status = UserStatus::Suspended;
    let cloned = status;
    assert_eq!(status, cloned);
}

#[test]
fn user_status_copy() {
    let status = UserStatus::Pending;
    let copied = status;
    assert_eq!(status, copied);
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

#[test]
fn user_status_deserializes_from_snake_case() {
    let status: UserStatus = serde_json::from_str("\"active\"").unwrap();
    assert_eq!(status, UserStatus::Active);
    let status: UserStatus = serde_json::from_str("\"suspended\"").unwrap();
    assert_eq!(status, UserStatus::Suspended);
}

#[test]
fn user_status_all_variants_round_trip() {
    let variants = [
        UserStatus::Active, UserStatus::Inactive, UserStatus::Suspended,
        UserStatus::Pending, UserStatus::Deleted, UserStatus::Temporary,
    ];
    for variant in variants {
        let json = serde_json::to_string(&variant).unwrap();
        let deserialized: UserStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, deserialized);
    }
}
