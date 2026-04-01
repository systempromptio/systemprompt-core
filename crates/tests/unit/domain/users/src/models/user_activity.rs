//! Unit tests for UserActivity and UserWithSessions structs

use chrono::Utc;
use systemprompt_users::{UserActivity, UserWithSessions};
use systemprompt_identifiers::UserId;

#[test]
fn user_activity_creation() {
    let activity = UserActivity {
        user_id: UserId::new("user-123".to_string()), last_active: Some(Utc::now()),
        session_count: 5, task_count: 10, message_count: 100,
    };
    assert_eq!(activity.session_count, 5);
    assert_eq!(activity.task_count, 10);
    assert_eq!(activity.message_count, 100);
}

#[test]
fn user_activity_clone() {
    let activity = UserActivity {
        user_id: UserId::new("user-123".to_string()), last_active: Some(Utc::now()),
        session_count: 3, task_count: 7, message_count: 50,
    };
    let cloned = activity.clone();
    assert_eq!(activity.user_id.to_string(), cloned.user_id.to_string());
    assert_eq!(activity.session_count, cloned.session_count);
}

#[test]
fn user_activity_debug() {
    let activity = UserActivity {
        user_id: UserId::new("user-123".to_string()), last_active: None,
        session_count: 0, task_count: 0, message_count: 0,
    };
    assert!(format!("{:?}", activity).contains("UserActivity"));
}

#[test]
fn user_activity_serialization_roundtrip() {
    let activity = UserActivity {
        user_id: UserId::new("user-123".to_string()), last_active: Some(Utc::now()),
        session_count: 5, task_count: 10, message_count: 100,
    };
    let json = serde_json::to_string(&activity).unwrap();
    let deserialized: UserActivity = serde_json::from_str(&json).unwrap();
    assert_eq!(activity.user_id.to_string(), deserialized.user_id.to_string());
    assert_eq!(activity.session_count, deserialized.session_count);
}

#[test]
fn user_activity_with_no_last_active() {
    let activity = UserActivity {
        user_id: UserId::new("user-123".to_string()), last_active: None,
        session_count: 0, task_count: 0, message_count: 0,
    };
    assert!(activity.last_active.is_none());
}

#[test]
fn user_with_sessions_creation() {
    let user = UserWithSessions {
        id: UserId::new("user-123".to_string()), name: "testuser".to_string(),
        email: "test@example.com".to_string(), full_name: Some("Test User".to_string()),
        status: Some("active".to_string()), roles: vec!["user".to_string()],
        created_at: Some(Utc::now()), active_sessions: 3, last_session_at: Some(Utc::now()),
    };
    assert_eq!(user.active_sessions, 3);
    user.last_session_at.expect("expected Some value");
}

#[test]
fn user_with_sessions_clone() {
    let user = UserWithSessions {
        id: UserId::new("user-123".to_string()), name: "testuser".to_string(),
        email: "test@example.com".to_string(), full_name: None, status: None,
        roles: vec![], created_at: None, active_sessions: 0, last_session_at: None,
    };
    let cloned = user.clone();
    assert_eq!(user.id.to_string(), cloned.id.to_string());
    assert_eq!(user.name, cloned.name);
}

#[test]
fn user_with_sessions_debug() {
    let user = UserWithSessions {
        id: UserId::new("user-123".to_string()), name: "testuser".to_string(),
        email: "test@example.com".to_string(), full_name: None, status: None,
        roles: vec![], created_at: None, active_sessions: 0, last_session_at: None,
    };
    assert!(format!("{:?}", user).contains("UserWithSessions"));
}

#[test]
fn user_with_sessions_serialization_roundtrip() {
    let user = UserWithSessions {
        id: UserId::new("user-123".to_string()), name: "testuser".to_string(),
        email: "test@example.com".to_string(), full_name: Some("Test User".to_string()),
        status: Some("active".to_string()), roles: vec!["user".to_string(), "admin".to_string()],
        created_at: Some(Utc::now()), active_sessions: 5, last_session_at: Some(Utc::now()),
    };
    let json = serde_json::to_string(&user).unwrap();
    let deserialized: UserWithSessions = serde_json::from_str(&json).unwrap();
    assert_eq!(user.id.to_string(), deserialized.id.to_string());
    assert_eq!(user.active_sessions, deserialized.active_sessions);
}

#[test]
fn user_with_sessions_no_active_sessions() {
    let user = UserWithSessions {
        id: UserId::new("user-123".to_string()), name: "testuser".to_string(),
        email: "test@example.com".to_string(), full_name: None, status: None,
        roles: vec![], created_at: None, active_sessions: 0, last_session_at: None,
    };
    assert_eq!(user.active_sessions, 0);
    assert!(user.last_session_at.is_none());
}
