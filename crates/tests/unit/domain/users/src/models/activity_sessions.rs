//! Unit tests for UserActivity, UserWithSessions, and UserSession

use chrono::Utc;
use systemprompt_users::{User, UserRole, UserStatus};
use systemprompt_identifiers::UserId;

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
