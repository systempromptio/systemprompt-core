//! Unit tests for UserExport struct and User -> UserExport conversion

use chrono::Utc;
use systemprompt_identifiers::UserId;
use systemprompt_users::{User, UserExport};

fn create_test_user_export() -> UserExport {
    UserExport {
        id: "user-export-123".to_string(),
        name: "exportuser".to_string(),
        email: "export@example.com".to_string(),
        full_name: Some("Export User".to_string()),
        display_name: Some("Export".to_string()),
        status: Some("active".to_string()),
        email_verified: Some(true),
        roles: vec!["user".to_string()],
        is_bot: false,
        is_scanner: false,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    }
}

#[test]
fn user_export_creation() {
    let export = create_test_user_export();

    assert_eq!(export.id, "user-export-123");
    assert_eq!(export.name, "exportuser");
    assert_eq!(export.email, "export@example.com");
}

#[test]
fn user_export_clone() {
    let export = create_test_user_export();
    let cloned = export.clone();

    assert_eq!(export.id, cloned.id);
    assert_eq!(export.name, cloned.name);
}

#[test]
fn user_export_debug() {
    let export = create_test_user_export();
    let debug = format!("{:?}", export);

    assert!(debug.contains("UserExport"));
}

#[test]
fn user_export_serialization_roundtrip() {
    let export = create_test_user_export();
    let json = serde_json::to_string(&export).unwrap();
    let deserialized: UserExport = serde_json::from_str(&json).unwrap();

    assert_eq!(export.id, deserialized.id);
    assert_eq!(export.name, deserialized.name);
    assert_eq!(export.email, deserialized.email);
}

#[test]
fn user_export_from_user_conversion() {
    let user = User {
        id: UserId::new("user-456".to_string()),
        name: "testuser".to_string(),
        email: "test@example.com".to_string(),
        full_name: Some("Test User".to_string()),
        display_name: Some("Test".to_string()),
        status: Some("active".to_string()),
        email_verified: Some(true),
        roles: vec!["user".to_string(), "admin".to_string()],
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        is_bot: false,
        is_scanner: true,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };

    let export: UserExport = user.clone().into();

    assert_eq!(export.id, user.id.to_string());
    assert_eq!(export.name, user.name);
    assert_eq!(export.email, user.email);
    assert_eq!(export.full_name, user.full_name);
    assert_eq!(export.display_name, user.display_name);
    assert_eq!(export.status, user.status);
    assert_eq!(export.email_verified, user.email_verified);
    assert_eq!(export.roles, user.roles);
    assert_eq!(export.is_bot, user.is_bot);
    assert_eq!(export.is_scanner, user.is_scanner);
}

#[test]
fn user_export_from_user_with_none_fields() {
    let user = User {
        id: UserId::new("user-789".to_string()),
        name: "minimal".to_string(),
        email: "minimal@example.com".to_string(),
        full_name: None,
        display_name: None,
        status: None,
        email_verified: None,
        roles: vec![],
        avatar_url: None,
        is_bot: true,
        is_scanner: false,
        created_at: None,
        updated_at: None,
    };

    let export: UserExport = user.into();

    assert!(export.full_name.is_none());
    assert!(export.display_name.is_none());
    assert!(export.status.is_none());
    assert!(export.email_verified.is_none());
    assert!(export.created_at.is_none());
    assert!(export.updated_at.is_none());
    assert!(export.is_bot);
}

#[test]
fn user_export_with_empty_roles() {
    let export = UserExport {
        id: "user-empty-roles".to_string(),
        name: "emptyroles".to_string(),
        email: "empty@example.com".to_string(),
        full_name: None,
        display_name: None,
        status: None,
        email_verified: None,
        roles: vec![],
        is_bot: false,
        is_scanner: false,
        created_at: None,
        updated_at: None,
    };

    assert!(export.roles.is_empty());
}

#[test]
fn user_export_json_includes_all_fields() {
    let export = create_test_user_export();
    let json = serde_json::to_string(&export).unwrap();

    assert!(json.contains("id"));
    assert!(json.contains("name"));
    assert!(json.contains("email"));
    assert!(json.contains("full_name"));
    assert!(json.contains("display_name"));
    assert!(json.contains("status"));
    assert!(json.contains("email_verified"));
    assert!(json.contains("roles"));
    assert!(json.contains("is_bot"));
    assert!(json.contains("is_scanner"));
}
