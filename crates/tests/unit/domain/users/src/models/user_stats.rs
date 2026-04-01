//! Unit tests for UserStats, UserCountBreakdown, and UserExport

use chrono::Utc;
use systemprompt_users::{User, UserStats, UserCountBreakdown, UserExport};
use systemprompt_identifiers::UserId;
use std::collections::HashMap;

#[test]
fn user_stats_creation() {
    let stats = UserStats { total: 100, created_24h: 5, created_7d: 20, created_30d: 50, active: 80, suspended: 10, admins: 3, anonymous: 15, bots: 2, oldest_user: Some(Utc::now()), newest_user: Some(Utc::now()) };
    assert_eq!(stats.total, 100); assert_eq!(stats.created_24h, 5); assert_eq!(stats.active, 80); assert_eq!(stats.admins, 3);
}

#[test]
fn user_stats_clone() {
    let stats = UserStats { total: 50, created_24h: 2, created_7d: 10, created_30d: 25, active: 40, suspended: 5, admins: 2, anonymous: 8, bots: 1, oldest_user: None, newest_user: None };
    let cloned = stats; assert_eq!(stats.total, cloned.total); assert_eq!(stats.active, cloned.active);
}

#[test]
fn user_stats_debug() {
    let stats = UserStats { total: 100, created_24h: 5, created_7d: 20, created_30d: 50, active: 80, suspended: 10, admins: 3, anonymous: 15, bots: 2, oldest_user: None, newest_user: None };
    assert!(format!("{:?}", stats).contains("UserStats"));
}

#[test]
fn user_stats_with_no_dates() {
    let stats = UserStats { total: 0, created_24h: 0, created_7d: 0, created_30d: 0, active: 0, suspended: 0, admins: 0, anonymous: 0, bots: 0, oldest_user: None, newest_user: None };
    assert!(stats.oldest_user.is_none()); assert!(stats.newest_user.is_none());
}

#[test]
fn user_stats_json_includes_all_fields() {
    let stats = UserStats { total: 100, created_24h: 5, created_7d: 20, created_30d: 50, active: 80, suspended: 10, admins: 3, anonymous: 15, bots: 2, oldest_user: None, newest_user: None };
    let json = serde_json::to_string(&stats).unwrap();
    for field in ["total", "created_24h", "created_7d", "created_30d", "active", "suspended", "admins", "anonymous", "bots"] {
        assert!(json.contains(field));
    }
}

#[test]
fn user_count_breakdown_creation() {
    let mut by_status = HashMap::new(); by_status.insert("active".to_string(), 80); by_status.insert("suspended".to_string(), 10);
    let mut by_role = HashMap::new(); by_role.insert("user".to_string(), 85); by_role.insert("admin".to_string(), 5);
    let breakdown = UserCountBreakdown { total: 100, by_status, by_role };
    assert_eq!(breakdown.total, 100); assert_eq!(breakdown.by_status.get("active"), Some(&80)); assert_eq!(breakdown.by_role.get("admin"), Some(&5));
}

#[test]
fn user_count_breakdown_debug() {
    let breakdown = UserCountBreakdown { total: 100, by_status: HashMap::new(), by_role: HashMap::new() };
    assert!(format!("{:?}", breakdown).contains("UserCountBreakdown"));
}

#[test]
fn user_count_breakdown_empty_maps() {
    let breakdown = UserCountBreakdown { total: 0, by_status: HashMap::new(), by_role: HashMap::new() };
    assert!(breakdown.by_status.is_empty()); assert!(breakdown.by_role.is_empty());
}

#[test]
fn user_count_breakdown_multiple_statuses() {
    let mut by_status = HashMap::new();
    for (k, v) in [("active", 50), ("inactive", 20), ("suspended", 10), ("pending", 15), ("deleted", 5)] {
        by_status.insert(k.to_string(), v);
    }
    let breakdown = UserCountBreakdown { total: 100, by_status, by_role: HashMap::new() };
    assert_eq!(breakdown.by_status.len(), 5);
}

fn create_test_user_export() -> UserExport {
    UserExport { id: "user-export-123".to_string(), name: "exportuser".to_string(), email: "export@example.com".to_string(), full_name: Some("Export User".to_string()), display_name: Some("Export".to_string()), status: Some("active".to_string()), email_verified: Some(true), roles: vec!["user".to_string()], is_bot: false, is_scanner: false, created_at: Some(Utc::now()), updated_at: Some(Utc::now()) }
}

#[test] fn user_export_creation() { let e = create_test_user_export(); assert_eq!(e.id, "user-export-123"); assert_eq!(e.name, "exportuser"); assert_eq!(e.email, "export@example.com"); }
#[test] fn user_export_debug() { assert!(format!("{:?}", create_test_user_export()).contains("UserExport")); }

#[test]
fn user_export_from_user_conversion() {
    let user = User { id: UserId::new("user-456".to_string()), name: "testuser".to_string(), email: "test@example.com".to_string(), full_name: Some("Test User".to_string()), display_name: Some("Test".to_string()), status: Some("active".to_string()), email_verified: Some(true), roles: vec!["user".to_string(), "admin".to_string()], avatar_url: Some("https://example.com/avatar.png".to_string()), is_bot: false, is_scanner: true, created_at: Some(Utc::now()), updated_at: Some(Utc::now()) };
    let export: UserExport = user.clone().into();
    assert_eq!(export.id, user.id.to_string()); assert_eq!(export.name, user.name); assert_eq!(export.email, user.email);
    assert_eq!(export.full_name, user.full_name); assert_eq!(export.display_name, user.display_name);
    assert_eq!(export.status, user.status); assert_eq!(export.email_verified, user.email_verified);
    assert_eq!(export.roles, user.roles); assert_eq!(export.is_bot, user.is_bot); assert_eq!(export.is_scanner, user.is_scanner);
}

#[test]
fn user_export_from_user_with_none_fields() {
    let user = User { id: UserId::new("user-789".to_string()), name: "minimal".to_string(), email: "minimal@example.com".to_string(), full_name: None, display_name: None, status: None, email_verified: None, roles: vec![], avatar_url: None, is_bot: true, is_scanner: false, created_at: None, updated_at: None };
    let export: UserExport = user.into();
    assert!(export.full_name.is_none()); assert!(export.display_name.is_none()); assert!(export.status.is_none());
    assert!(export.email_verified.is_none()); assert!(export.created_at.is_none()); assert!(export.updated_at.is_none()); assert!(export.is_bot);
}

#[test]
fn user_export_with_empty_roles() {
    let export = UserExport { id: "user-empty-roles".to_string(), name: "emptyroles".to_string(), email: "empty@example.com".to_string(), full_name: None, display_name: None, status: None, email_verified: None, roles: vec![], is_bot: false, is_scanner: false, created_at: None, updated_at: None };
    assert!(export.roles.is_empty());
}

#[test]
fn user_export_json_includes_all_fields() {
    let json = serde_json::to_string(&create_test_user_export()).unwrap();
    for field in ["id", "name", "email", "full_name", "display_name", "status", "email_verified", "roles", "is_bot", "is_scanner"] {
        assert!(json.contains(field));
    }
}
