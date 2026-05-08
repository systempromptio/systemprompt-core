//! Tests for DatabaseExport struct

use chrono::{TimeZone, Utc};
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_sync::{ContextExport, DatabaseExport};

#[test]
fn full_export() {
    let now = Utc::now();
    let export = DatabaseExport {
        users: vec![],
        contexts: vec![ContextExport {
            context_id: ContextId::new("ctx_1"),
            user_id: UserId::new("user_1"),
            session_id: None,
            name: "Context".to_string(),
            created_at: now,
            updated_at: now,
        }],
        timestamp: now,
    };

    assert_eq!(export.contexts.len(), 1);
}

#[test]
fn empty_export() {
    let export = DatabaseExport {
        users: vec![],
        contexts: vec![],
        timestamp: Utc::now(),
    };

    assert!(export.users.is_empty());
    assert!(export.contexts.is_empty());
}

#[test]
fn serialization() {
    let now = Utc
        .with_ymd_and_hms(2024, 1, 15, 12, 0, 0)
        .single()
        .expect("valid datetime");
    let export = DatabaseExport {
        users: vec![],
        contexts: vec![],
        timestamp: now,
    };

    let json = serde_json::to_string(&export).expect("serialize database export");
    assert!(json.contains("\"contexts\":[]"));
}

#[test]
fn with_users() {
    use systemprompt_sync::database::UserExport;

    let now = Utc::now();
    let export = DatabaseExport {
        users: vec![
            UserExport {
                id: UserId::new("user_1"),
                name: "user1".to_string(),
                email: "user1@example.com".to_string(),
                full_name: Some("User One".to_string()),
                display_name: None,
                status: "active".to_string(),
                email_verified: true,
                roles: vec!["admin".to_string()],
                is_bot: false,
                is_scanner: false,
                avatar_url: None,
                created_at: now,
                updated_at: now,
            },
            UserExport {
                id: UserId::new("user_2"),
                name: "user2".to_string(),
                email: "user2@example.com".to_string(),
                full_name: None,
                display_name: Some("U2".to_string()),
                status: "pending".to_string(),
                email_verified: false,
                roles: vec![],
                is_bot: true,
                is_scanner: false,
                avatar_url: Some("https://example.com/u2.png".to_string()),
                created_at: now,
                updated_at: now,
            },
        ],
        contexts: vec![],
        timestamp: now,
    };

    assert_eq!(export.users.len(), 2);
    assert_eq!(export.users[0].name, "user1");
    assert_eq!(export.users[1].name, "user2");
}
