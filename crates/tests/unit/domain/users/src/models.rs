//! Unit tests for user models.
//!
//! Tests cover:
//! - UserStatus enum and its methods
//! - UserRole enum and its methods
//! - User struct helper methods
//! - UserActivity, UserWithSessions, UserSession structs
//! - UserSessionRow to UserSession conversion

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

// ============================================================================
// User Tests
// ============================================================================

mod user_tests {
    use super::*;

    fn create_test_user() -> User {
        User {
            id: UserId::new("user-123".to_string()),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            display_name: Some("Test".to_string()),
            status: Some("active".to_string()),
            email_verified: Some(true),
            roles: vec!["user".to_string()],
            avatar_url: Some("https://example.com/avatar.png".to_string()),
            is_bot: false,
            is_scanner: false,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }

    #[test]
    fn is_active_returns_true_for_active_status() {
        let user = create_test_user();
        assert!(user.is_active());
    }

    #[test]
    fn is_active_returns_false_for_inactive_status() {
        let mut user = create_test_user();
        user.status = Some("inactive".to_string());
        assert!(!user.is_active());
    }

    #[test]
    fn is_active_returns_false_for_suspended_status() {
        let mut user = create_test_user();
        user.status = Some("suspended".to_string());
        assert!(!user.is_active());
    }

    #[test]
    fn is_active_returns_false_for_none_status() {
        let mut user = create_test_user();
        user.status = None;
        assert!(!user.is_active());
    }

    #[test]
    fn is_admin_returns_true_when_has_admin_role() {
        let mut user = create_test_user();
        user.roles = vec!["admin".to_string(), "user".to_string()];
        assert!(user.is_admin());
    }

    #[test]
    fn is_admin_returns_false_when_no_admin_role() {
        let user = create_test_user();
        assert!(!user.is_admin());
    }

    #[test]
    fn is_admin_returns_false_for_empty_roles() {
        let mut user = create_test_user();
        user.roles = vec![];
        assert!(!user.is_admin());
    }

    #[test]
    fn has_role_returns_true_for_existing_role() {
        let user = create_test_user();
        assert!(user.has_role(UserRole::User));
    }

    #[test]
    fn has_role_returns_false_for_missing_role() {
        let user = create_test_user();
        assert!(!user.has_role(UserRole::Admin));
    }

    #[test]
    fn has_role_returns_true_for_admin_role() {
        let mut user = create_test_user();
        user.roles = vec!["admin".to_string()];
        assert!(user.has_role(UserRole::Admin));
    }

    #[test]
    fn has_role_returns_true_for_anonymous_role() {
        let mut user = create_test_user();
        user.roles = vec!["anonymous".to_string()];
        assert!(user.has_role(UserRole::Anonymous));
    }

    #[test]
    fn has_role_returns_false_for_empty_roles() {
        let mut user = create_test_user();
        user.roles = vec![];
        assert!(!user.has_role(UserRole::User));
    }

    #[test]
    fn user_clone() {
        let user = create_test_user();
        let cloned = user.clone();
        assert_eq!(user.id.to_string(), cloned.id.to_string());
        assert_eq!(user.name, cloned.name);
        assert_eq!(user.email, cloned.email);
    }

    #[test]
    fn user_debug() {
        let user = create_test_user();
        let debug_str = format!("{:?}", user);
        assert!(debug_str.contains("User"));
        assert!(debug_str.contains("testuser"));
    }

    #[test]
    fn user_serialization_roundtrip() {
        let user = create_test_user();
        let json = serde_json::to_string(&user).unwrap();
        let deserialized: User = serde_json::from_str(&json).unwrap();

        assert_eq!(user.id.to_string(), deserialized.id.to_string());
        assert_eq!(user.name, deserialized.name);
        assert_eq!(user.email, deserialized.email);
        assert_eq!(user.roles, deserialized.roles);
    }

    #[test]
    fn user_with_multiple_roles() {
        let mut user = create_test_user();
        user.roles = vec![
            "admin".to_string(),
            "user".to_string(),
            "moderator".to_string(),
        ];

        assert!(user.has_role(UserRole::Admin));
        assert!(user.has_role(UserRole::User));
        assert!(user.is_admin());
    }

    #[test]
    fn user_is_bot_field() {
        let mut user = create_test_user();
        assert!(!user.is_bot);

        user.is_bot = true;
        assert!(user.is_bot);
    }

    #[test]
    fn user_is_scanner_field() {
        let mut user = create_test_user();
        assert!(!user.is_scanner);

        user.is_scanner = true;
        assert!(user.is_scanner);
    }

    #[test]
    fn user_optional_fields_can_be_none() {
        let user = User {
            id: UserId::new("user-456".to_string()),
            name: "minimal".to_string(),
            email: "minimal@example.com".to_string(),
            full_name: None,
            display_name: None,
            status: None,
            email_verified: None,
            roles: vec![],
            avatar_url: None,
            is_bot: false,
            is_scanner: false,
            created_at: None,
            updated_at: None,
        };

        assert!(user.full_name.is_none());
        assert!(user.display_name.is_none());
        assert!(user.status.is_none());
        assert!(user.email_verified.is_none());
        assert!(user.avatar_url.is_none());
        assert!(user.created_at.is_none());
        assert!(user.updated_at.is_none());
    }
}

// ============================================================================
// UserActivity Tests
// ============================================================================

mod user_activity_tests {
    use super::*;
    use systemprompt_users::UserActivity;

    #[test]
    fn user_activity_creation() {
        let activity = UserActivity {
            user_id: UserId::new("user-123".to_string()),
            last_active: Some(Utc::now()),
            session_count: 5,
            task_count: 10,
            message_count: 100,
        };

        assert_eq!(activity.session_count, 5);
        assert_eq!(activity.task_count, 10);
        assert_eq!(activity.message_count, 100);
    }

    #[test]
    fn user_activity_clone() {
        let activity = UserActivity {
            user_id: UserId::new("user-123".to_string()),
            last_active: Some(Utc::now()),
            session_count: 3,
            task_count: 7,
            message_count: 50,
        };

        let cloned = activity.clone();
        assert_eq!(
            activity.user_id.to_string(),
            cloned.user_id.to_string()
        );
        assert_eq!(activity.session_count, cloned.session_count);
    }

    #[test]
    fn user_activity_debug() {
        let activity = UserActivity {
            user_id: UserId::new("user-123".to_string()),
            last_active: None,
            session_count: 0,
            task_count: 0,
            message_count: 0,
        };

        let debug_str = format!("{:?}", activity);
        assert!(debug_str.contains("UserActivity"));
    }

    #[test]
    fn user_activity_serialization_roundtrip() {
        let activity = UserActivity {
            user_id: UserId::new("user-123".to_string()),
            last_active: Some(Utc::now()),
            session_count: 5,
            task_count: 10,
            message_count: 100,
        };

        let json = serde_json::to_string(&activity).unwrap();
        let deserialized: UserActivity = serde_json::from_str(&json).unwrap();

        assert_eq!(
            activity.user_id.to_string(),
            deserialized.user_id.to_string()
        );
        assert_eq!(activity.session_count, deserialized.session_count);
    }

    #[test]
    fn user_activity_with_no_last_active() {
        let activity = UserActivity {
            user_id: UserId::new("user-123".to_string()),
            last_active: None,
            session_count: 0,
            task_count: 0,
            message_count: 0,
        };

        assert!(activity.last_active.is_none());
    }
}

// ============================================================================
// UserWithSessions Tests
// ============================================================================

mod user_with_sessions_tests {
    use super::*;
    use systemprompt_users::UserWithSessions;

    #[test]
    fn user_with_sessions_creation() {
        let user = UserWithSessions {
            id: UserId::new("user-123".to_string()),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            status: Some("active".to_string()),
            roles: vec!["user".to_string()],
            created_at: Some(Utc::now()),
            active_sessions: 3,
            last_session_at: Some(Utc::now()),
        };

        assert_eq!(user.active_sessions, 3);
        assert!(user.last_session_at.is_some());
    }

    #[test]
    fn user_with_sessions_clone() {
        let user = UserWithSessions {
            id: UserId::new("user-123".to_string()),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: None,
            status: None,
            roles: vec![],
            created_at: None,
            active_sessions: 0,
            last_session_at: None,
        };

        let cloned = user.clone();
        assert_eq!(user.id.to_string(), cloned.id.to_string());
        assert_eq!(user.name, cloned.name);
    }

    #[test]
    fn user_with_sessions_debug() {
        let user = UserWithSessions {
            id: UserId::new("user-123".to_string()),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: None,
            status: None,
            roles: vec![],
            created_at: None,
            active_sessions: 0,
            last_session_at: None,
        };

        let debug_str = format!("{:?}", user);
        assert!(debug_str.contains("UserWithSessions"));
    }

    #[test]
    fn user_with_sessions_serialization_roundtrip() {
        let user = UserWithSessions {
            id: UserId::new("user-123".to_string()),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            status: Some("active".to_string()),
            roles: vec!["user".to_string(), "admin".to_string()],
            created_at: Some(Utc::now()),
            active_sessions: 5,
            last_session_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&user).unwrap();
        let deserialized: UserWithSessions = serde_json::from_str(&json).unwrap();

        assert_eq!(user.id.to_string(), deserialized.id.to_string());
        assert_eq!(user.active_sessions, deserialized.active_sessions);
    }

    #[test]
    fn user_with_sessions_no_active_sessions() {
        let user = UserWithSessions {
            id: UserId::new("user-123".to_string()),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: None,
            status: None,
            roles: vec![],
            created_at: None,
            active_sessions: 0,
            last_session_at: None,
        };

        assert_eq!(user.active_sessions, 0);
        assert!(user.last_session_at.is_none());
    }
}

// ============================================================================
// UserSession Tests
// ============================================================================

mod user_session_tests {
    use super::*;
    use systemprompt_users::UserSession;
    use systemprompt_identifiers::SessionId;

    #[test]
    fn user_session_creation() {
        let session = UserSession {
            session_id: SessionId::new("session-123".to_string()),
            user_id: Some(UserId::new("user-123".to_string())),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
            device_type: Some("desktop".to_string()),
            started_at: Some(Utc::now()),
            last_activity_at: Some(Utc::now()),
            ended_at: None,
        };

        assert!(session.user_id.is_some());
        assert!(session.ended_at.is_none());
    }

    #[test]
    fn user_session_clone() {
        let session = UserSession {
            session_id: SessionId::new("session-123".to_string()),
            user_id: None,
            ip_address: None,
            user_agent: None,
            device_type: None,
            started_at: None,
            last_activity_at: None,
            ended_at: None,
        };

        let cloned = session.clone();
        assert_eq!(
            session.session_id.to_string(),
            cloned.session_id.to_string()
        );
    }

    #[test]
    fn user_session_debug() {
        let session = UserSession {
            session_id: SessionId::new("session-123".to_string()),
            user_id: None,
            ip_address: None,
            user_agent: None,
            device_type: None,
            started_at: None,
            last_activity_at: None,
            ended_at: None,
        };

        let debug_str = format!("{:?}", session);
        assert!(debug_str.contains("UserSession"));
    }

    #[test]
    fn user_session_serialization_roundtrip() {
        let session = UserSession {
            session_id: SessionId::new("session-123".to_string()),
            user_id: Some(UserId::new("user-123".to_string())),
            ip_address: Some("10.0.0.1".to_string()),
            user_agent: Some("Test Agent".to_string()),
            device_type: Some("mobile".to_string()),
            started_at: Some(Utc::now()),
            last_activity_at: Some(Utc::now()),
            ended_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: UserSession = serde_json::from_str(&json).unwrap();

        assert_eq!(
            session.session_id.to_string(),
            deserialized.session_id.to_string()
        );
        assert!(deserialized.ended_at.is_some());
    }

    #[test]
    fn user_session_active_when_ended_at_none() {
        let session = UserSession {
            session_id: SessionId::new("session-123".to_string()),
            user_id: Some(UserId::new("user-123".to_string())),
            ip_address: None,
            user_agent: None,
            device_type: None,
            started_at: Some(Utc::now()),
            last_activity_at: Some(Utc::now()),
            ended_at: None,
        };

        assert!(session.ended_at.is_none());
    }

    #[test]
    fn user_session_ended_when_ended_at_set() {
        let session = UserSession {
            session_id: SessionId::new("session-123".to_string()),
            user_id: Some(UserId::new("user-123".to_string())),
            ip_address: None,
            user_agent: None,
            device_type: None,
            started_at: Some(Utc::now()),
            last_activity_at: Some(Utc::now()),
            ended_at: Some(Utc::now()),
        };

        assert!(session.ended_at.is_some());
    }

    #[test]
    fn user_session_anonymous_when_user_id_none() {
        let session = UserSession {
            session_id: SessionId::new("session-anon".to_string()),
            user_id: None,
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: None,
            device_type: None,
            started_at: Some(Utc::now()),
            last_activity_at: None,
            ended_at: None,
        };

        assert!(session.user_id.is_none());
    }
}
