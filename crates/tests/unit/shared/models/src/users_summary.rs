use chrono::Utc;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::users::{SessionSummary, UserSummary};

#[test]
fn user_summary_fields_round_trip_via_serde() {
    let original = UserSummary {
        id: UserId::new("usr_1"),
        name: "Alice".to_owned(),
        email: "alice@example.com".to_owned(),
        status: Some("active".to_owned()),
        roles: vec!["admin".to_owned(), "user".to_owned()],
        created_at: Some(Utc::now()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let decoded: UserSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.id, original.id);
    assert_eq!(decoded.name, "Alice");
    assert_eq!(decoded.email, "alice@example.com");
    assert_eq!(decoded.status.as_deref(), Some("active"));
    assert_eq!(decoded.roles.len(), 2);
}

#[test]
fn user_summary_optional_fields_can_be_none() {
    let u = UserSummary {
        id: UserId::new("usr_2"),
        name: "Bob".to_owned(),
        email: "bob@example.com".to_owned(),
        status: None,
        roles: vec![],
        created_at: None,
    };
    let json = serde_json::to_string(&u).unwrap();
    let decoded: UserSummary = serde_json::from_str(&json).unwrap();
    assert!(decoded.status.is_none());
    assert!(decoded.created_at.is_none());
    assert!(decoded.roles.is_empty());
}

#[test]
fn session_summary_fields_round_trip_via_serde() {
    let original = SessionSummary {
        session_id: SessionId::generate(),
        ip_address: Some("127.0.0.1".to_owned()),
        user_agent: Some("Mozilla/5.0".to_owned()),
        device_type: Some("desktop".to_owned()),
        started_at: Some(Utc::now()),
        last_activity_at: Some(Utc::now()),
        is_active: true,
    };
    let json = serde_json::to_string(&original).unwrap();
    let decoded: SessionSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.session_id, original.session_id);
    assert_eq!(decoded.ip_address.as_deref(), Some("127.0.0.1"));
    assert!(decoded.is_active);
}

#[test]
fn session_summary_inactive_with_no_optional_fields() {
    let s = SessionSummary {
        session_id: SessionId::generate(),
        ip_address: None,
        user_agent: None,
        device_type: None,
        started_at: None,
        last_activity_at: None,
        is_active: false,
    };
    let json = serde_json::to_string(&s).unwrap();
    let decoded: SessionSummary = serde_json::from_str(&json).unwrap();
    assert!(!decoded.is_active);
    assert!(decoded.ip_address.is_none());
    assert!(decoded.user_agent.is_none());
}
