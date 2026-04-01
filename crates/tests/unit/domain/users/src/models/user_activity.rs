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
fn user_activity_debug() {
    let activity = UserActivity {
        user_id: UserId::new("user-123".to_string()), last_active: None,
        session_count: 0, task_count: 0, message_count: 0,
    };
    assert!(format!("{:?}", activity).contains("UserActivity"));
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
fn user_with_sessions_debug() {
    let user = UserWithSessions {
        id: UserId::new("user-123".to_string()), name: "testuser".to_string(),
        email: "test@example.com".to_string(), full_name: None, status: None,
        roles: vec![], created_at: None, active_sessions: 0, last_session_at: None,
    };
    assert!(format!("{:?}", user).contains("UserWithSessions"));
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
