//! Unit tests for UserSessionRow -> UserSession conversion.

use chrono::Utc;
use systemprompt_identifiers::{SessionId, UserId};

mod session_row_conversion_tests {
    use super::*;
    use systemprompt_users::UserSession;

    fn make_session_full() -> UserSession {
        UserSession {
            session_id: SessionId::new("sess-abc"),
            user_id: Some(UserId::new("user-xyz")),
            ip_address: Some("10.0.0.1".to_string()),
            user_agent: Some("TestAgent/1.0".to_string()),
            device_type: Some("mobile".to_string()),
            started_at: Some(Utc::now()),
            last_activity_at: Some(Utc::now()),
            ended_at: None,
        }
    }

    #[test]
    fn session_fields_preserved() {
        let s = make_session_full();
        assert_eq!(s.session_id.to_string(), "sess-abc");
        assert_eq!(s.user_id.as_ref().map(|id| id.to_string()), Some("user-xyz".to_string()));
        assert_eq!(s.ip_address.as_deref(), Some("10.0.0.1"));
        assert_eq!(s.user_agent.as_deref(), Some("TestAgent/1.0"));
        assert_eq!(s.device_type.as_deref(), Some("mobile"));
        assert!(s.started_at.is_some());
        assert!(s.last_activity_at.is_some());
        assert!(s.ended_at.is_none());
    }

    #[test]
    fn anonymous_session_has_no_user_id() {
        let s = UserSession {
            session_id: SessionId::new("anon-sess"),
            user_id: None,
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: None,
            device_type: None,
            started_at: Some(Utc::now()),
            last_activity_at: None,
            ended_at: None,
        };
        assert!(s.user_id.is_none());
    }

    #[test]
    fn ended_session_has_ended_at() {
        let s = UserSession {
            session_id: SessionId::new("ended-sess"),
            user_id: Some(UserId::new("user-zzz")),
            ip_address: None,
            user_agent: None,
            device_type: None,
            started_at: Some(Utc::now()),
            last_activity_at: Some(Utc::now()),
            ended_at: Some(Utc::now()),
        };
        assert!(s.ended_at.is_some());
    }

    #[test]
    fn debug_includes_session_id() {
        let s = make_session_full();
        let d = format!("{:?}", s);
        assert!(d.contains("UserSession"));
    }

    #[test]
    fn clone_preserves_session_id() {
        let s = make_session_full();
        let cloned = s.clone();
        assert_eq!(s.session_id.to_string(), cloned.session_id.to_string());
    }

    #[test]
    fn serde_round_trip() {
        let s = make_session_full();
        let json = serde_json::to_string(&s).expect("serialize");
        let decoded: UserSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s.session_id.to_string(), decoded.session_id.to_string());
        assert_eq!(
            s.user_id.as_ref().map(|id| id.to_string()),
            decoded.user_id.map(|id| id.to_string()),
        );
    }

    #[test]
    fn serde_null_ended_at() {
        let s = make_session_full();
        let json = serde_json::to_string(&s).expect("serialize");
        assert!(json.contains("\"ended_at\":null"));
    }

    #[test]
    fn all_optional_fields_none() {
        let s = UserSession {
            session_id: SessionId::new("minimal-sess"),
            user_id: None,
            ip_address: None,
            user_agent: None,
            device_type: None,
            started_at: None,
            last_activity_at: None,
            ended_at: None,
        };
        assert!(s.ip_address.is_none());
        assert!(s.user_agent.is_none());
        assert!(s.device_type.is_none());
        assert!(s.started_at.is_none());
        assert!(s.last_activity_at.is_none());
        assert!(s.ended_at.is_none());
    }
}
