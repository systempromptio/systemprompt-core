//! Unit tests for UserSession struct

use chrono::Utc;
use systemprompt_users::UserSession;
use systemprompt_identifiers::{SessionId, UserId};

#[test]
fn user_session_creation() {
    let session = UserSession {
        session_id: SessionId::new("session-123".to_string()),
        user_id: Some(UserId::new("user-123".to_string())),
        ip_address: Some("192.168.1.1".to_string()),
        user_agent: Some("Mozilla/5.0".to_string()),
        device_type: Some("desktop".to_string()),
        started_at: Some(Utc::now()), last_activity_at: Some(Utc::now()), ended_at: None,
    };
    session.user_id.expect("expected Some value");
    assert!(session.ended_at.is_none());
}

#[test]
fn user_session_clone() {
    let session = UserSession {
        session_id: SessionId::new("session-123".to_string()), user_id: None,
        ip_address: None, user_agent: None, device_type: None,
        started_at: None, last_activity_at: None, ended_at: None,
    };
    let cloned = session.clone();
    assert_eq!(session.session_id.to_string(), cloned.session_id.to_string());
}

#[test]
fn user_session_debug() {
    let session = UserSession {
        session_id: SessionId::new("session-123".to_string()), user_id: None,
        ip_address: None, user_agent: None, device_type: None,
        started_at: None, last_activity_at: None, ended_at: None,
    };
    assert!(format!("{:?}", session).contains("UserSession"));
}

#[test]
fn user_session_serialization_roundtrip() {
    let session = UserSession {
        session_id: SessionId::new("session-123".to_string()),
        user_id: Some(UserId::new("user-123".to_string())),
        ip_address: Some("10.0.0.1".to_string()), user_agent: Some("Test Agent".to_string()),
        device_type: Some("mobile".to_string()), started_at: Some(Utc::now()),
        last_activity_at: Some(Utc::now()), ended_at: Some(Utc::now()),
    };
    let json = serde_json::to_string(&session).unwrap();
    let deserialized: UserSession = serde_json::from_str(&json).unwrap();
    assert_eq!(session.session_id.to_string(), deserialized.session_id.to_string());
    deserialized.ended_at.expect("expected Some value");
}

#[test]
fn user_session_active_when_ended_at_none() {
    let session = UserSession {
        session_id: SessionId::new("session-123".to_string()),
        user_id: Some(UserId::new("user-123".to_string())),
        ip_address: None, user_agent: None, device_type: None,
        started_at: Some(Utc::now()), last_activity_at: Some(Utc::now()), ended_at: None,
    };
    assert!(session.ended_at.is_none());
}

#[test]
fn user_session_ended_when_ended_at_set() {
    let session = UserSession {
        session_id: SessionId::new("session-123".to_string()),
        user_id: Some(UserId::new("user-123".to_string())),
        ip_address: None, user_agent: None, device_type: None,
        started_at: Some(Utc::now()), last_activity_at: Some(Utc::now()), ended_at: Some(Utc::now()),
    };
    session.ended_at.expect("expected Some value");
}

#[test]
fn user_session_anonymous_when_user_id_none() {
    let session = UserSession {
        session_id: SessionId::new("session-anon".to_string()), user_id: None,
        ip_address: Some("127.0.0.1".to_string()), user_agent: None, device_type: None,
        started_at: Some(Utc::now()), last_activity_at: None, ended_at: None,
    };
    assert!(session.user_id.is_none());
}
