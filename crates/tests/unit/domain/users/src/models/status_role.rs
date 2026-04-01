//! Unit tests for UserStatus and UserRole enums

use chrono::Utc;
use systemprompt_users::{User, UserRole, UserStatus};
use systemprompt_identifiers::UserId;

// ============================================================================
// UserStatus Tests
// ============================================================================

mod user_status_tests {
    use super::*;

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
            UserStatus::Active,
            UserStatus::Inactive,
            UserStatus::Suspended,
            UserStatus::Pending,
            UserStatus::Deleted,
            UserStatus::Temporary,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: UserStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }
}

// ============================================================================
// UserRole Tests
// ============================================================================

mod user_role_tests {
    use super::*;

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

    #[test]
    fn user_role_equality() {
        assert_eq!(UserRole::Admin, UserRole::Admin);
        assert_ne!(UserRole::Admin, UserRole::User);
        assert_ne!(UserRole::User, UserRole::Anonymous);
    }

    #[test]
    fn user_role_clone() {
        let role = UserRole::Admin;
        let cloned = role;
        assert_eq!(role, cloned);
    }

    #[test]
    fn user_role_copy() {
        let role = UserRole::User;
        let copied = role;
        assert_eq!(role, copied);
    }

    #[test]
    fn user_role_debug() {
        let debug_str = format!("{:?}", UserRole::Admin);
        assert!(debug_str.contains("Admin"));
    }

    #[test]
    fn user_role_serializes_to_snake_case() {
        let json = serde_json::to_string(&UserRole::Admin).unwrap();
        assert_eq!(json, "\"admin\"");

        let json = serde_json::to_string(&UserRole::User).unwrap();
        assert_eq!(json, "\"user\"");

        let json = serde_json::to_string(&UserRole::Anonymous).unwrap();
        assert_eq!(json, "\"anonymous\"");
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
        let variants = [UserRole::Admin, UserRole::User, UserRole::Anonymous];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: UserRole = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }
}
