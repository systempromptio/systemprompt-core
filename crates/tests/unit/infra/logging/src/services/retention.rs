//! Unit tests for RetentionPolicy and RetentionConfig

use systemprompt_core_logging::services::retention::{RetentionConfig, RetentionPolicy};
use systemprompt_core_logging::LogLevel;

// ============================================================================
// RetentionPolicy Creation Tests
// ============================================================================

#[test]
fn test_retention_policy_new() {
    let policy = RetentionPolicy::new("test_policy", 30);

    assert_eq!(policy.name, "test_policy");
    assert_eq!(policy.retention_days, 30);
    assert!(policy.level.is_none());
    assert!(policy.module.is_none());
}

#[test]
fn test_retention_policy_new_with_string() {
    let policy = RetentionPolicy::new(String::from("my_policy"), 7);

    assert_eq!(policy.name, "my_policy");
    assert_eq!(policy.retention_days, 7);
}

#[test]
fn test_retention_policy_with_level() {
    let policy = RetentionPolicy::new("error_policy", 90).with_level(LogLevel::Error);

    assert_eq!(policy.name, "error_policy");
    assert_eq!(policy.retention_days, 90);
    assert_eq!(policy.level, Some(LogLevel::Error));
    assert!(policy.module.is_none());
}

#[test]
fn test_retention_policy_with_module() {
    let policy = RetentionPolicy::new("auth_policy", 14).with_module("auth");

    assert_eq!(policy.name, "auth_policy");
    assert_eq!(policy.retention_days, 14);
    assert!(policy.level.is_none());
    assert_eq!(policy.module, Some("auth".to_string()));
}

#[test]
fn test_retention_policy_with_level_and_module() {
    let policy = RetentionPolicy::new("debug_auth", 1)
        .with_level(LogLevel::Debug)
        .with_module("auth::login");

    assert_eq!(policy.name, "debug_auth");
    assert_eq!(policy.retention_days, 1);
    assert_eq!(policy.level, Some(LogLevel::Debug));
    assert_eq!(policy.module, Some("auth::login".to_string()));
}

#[test]
fn test_retention_policy_all_log_levels() {
    let error = RetentionPolicy::new("error", 90).with_level(LogLevel::Error);
    let warn = RetentionPolicy::new("warn", 60).with_level(LogLevel::Warn);
    let info = RetentionPolicy::new("info", 30).with_level(LogLevel::Info);
    let debug = RetentionPolicy::new("debug", 7).with_level(LogLevel::Debug);
    let trace = RetentionPolicy::new("trace", 1).with_level(LogLevel::Trace);

    assert_eq!(error.level, Some(LogLevel::Error));
    assert_eq!(warn.level, Some(LogLevel::Warn));
    assert_eq!(info.level, Some(LogLevel::Info));
    assert_eq!(debug.level, Some(LogLevel::Debug));
    assert_eq!(trace.level, Some(LogLevel::Trace));
}

// ============================================================================
// RetentionPolicy Clone and Debug Tests
// ============================================================================

#[test]
fn test_retention_policy_clone() {
    let policy = RetentionPolicy::new("test", 30)
        .with_level(LogLevel::Info)
        .with_module("api");
    let cloned = policy.clone();

    assert_eq!(policy.name, cloned.name);
    assert_eq!(policy.retention_days, cloned.retention_days);
    assert_eq!(policy.level, cloned.level);
    assert_eq!(policy.module, cloned.module);
}

#[test]
fn test_retention_policy_debug() {
    let policy = RetentionPolicy::new("debug_test", 7).with_level(LogLevel::Debug);
    let debug = format!("{:?}", policy);

    assert!(debug.contains("RetentionPolicy"));
    assert!(debug.contains("debug_test"));
    assert!(debug.contains("7"));
}

// ============================================================================
// RetentionPolicy Serialization Tests
// ============================================================================

#[test]
fn test_retention_policy_serialize() {
    let policy = RetentionPolicy::new("serialize_test", 14).with_level(LogLevel::Warn);
    let json = serde_json::to_string(&policy).unwrap();

    assert!(json.contains("\"name\":\"serialize_test\""));
    assert!(json.contains("\"retention_days\":14"));
    assert!(json.contains("\"level\":\"WARN\""));
}

#[test]
fn test_retention_policy_deserialize() {
    let json = r#"{"name":"test","level":"ERROR","module":"api","retention_days":30}"#;
    let policy: RetentionPolicy = serde_json::from_str(json).unwrap();

    assert_eq!(policy.name, "test");
    assert_eq!(policy.level, Some(LogLevel::Error));
    assert_eq!(policy.module, Some("api".to_string()));
    assert_eq!(policy.retention_days, 30);
}

#[test]
fn test_retention_policy_roundtrip() {
    let policy = RetentionPolicy::new("roundtrip", 45)
        .with_level(LogLevel::Info)
        .with_module("database");

    let json = serde_json::to_string(&policy).unwrap();
    let parsed: RetentionPolicy = serde_json::from_str(&json).unwrap();

    assert_eq!(policy.name, parsed.name);
    assert_eq!(policy.retention_days, parsed.retention_days);
    assert_eq!(policy.level, parsed.level);
    assert_eq!(policy.module, parsed.module);
}

// ============================================================================
// RetentionConfig Default Tests
// ============================================================================

#[test]
fn test_retention_config_default() {
    let config = RetentionConfig::default();

    assert!(config.enabled);
    assert!(config.vacuum_after_cleanup);
    assert!(!config.policies.is_empty());
}

#[test]
fn test_retention_config_default_schedule() {
    let config = RetentionConfig::default();

    // Default schedule is "0 0 2 * * *" (2 AM daily)
    assert_eq!(config.schedule, "0 0 2 * * *");
}

#[test]
fn test_retention_config_default_policies() {
    let config = RetentionConfig::default();

    // Default has 4 policies: debug (1 day), info (7 days), warnings (30 days), errors (90 days)
    assert_eq!(config.policies.len(), 4);

    // Check debug logs policy
    let debug_policy = config.policies.iter().find(|p| p.name == "debug_logs");
    assert!(debug_policy.is_some());
    let debug_policy = debug_policy.unwrap();
    assert_eq!(debug_policy.retention_days, 1);
    assert_eq!(debug_policy.level, Some(LogLevel::Debug));

    // Check info logs policy
    let info_policy = config.policies.iter().find(|p| p.name == "info_logs");
    assert!(info_policy.is_some());
    let info_policy = info_policy.unwrap();
    assert_eq!(info_policy.retention_days, 7);
    assert_eq!(info_policy.level, Some(LogLevel::Info));

    // Check warnings policy
    let warn_policy = config.policies.iter().find(|p| p.name == "warnings");
    assert!(warn_policy.is_some());
    let warn_policy = warn_policy.unwrap();
    assert_eq!(warn_policy.retention_days, 30);
    assert_eq!(warn_policy.level, Some(LogLevel::Warn));

    // Check errors policy
    let error_policy = config.policies.iter().find(|p| p.name == "errors");
    assert!(error_policy.is_some());
    let error_policy = error_policy.unwrap();
    assert_eq!(error_policy.retention_days, 90);
    assert_eq!(error_policy.level, Some(LogLevel::Error));
}

// ============================================================================
// RetentionConfig Builder Tests
// ============================================================================

#[test]
fn test_retention_config_with_schedule() {
    let config = RetentionConfig::default().with_schedule("0 0 3 * * *");

    assert_eq!(config.schedule, "0 0 3 * * *");
}

#[test]
fn test_retention_config_with_schedule_string() {
    let config = RetentionConfig::default().with_schedule(String::from("0 30 1 * * *"));

    assert_eq!(config.schedule, "0 30 1 * * *");
}

#[test]
fn test_retention_config_add_policy() {
    let policy = RetentionPolicy::new("custom_policy", 60).with_level(LogLevel::Warn);
    let config = RetentionConfig::default().add_policy(policy);

    assert_eq!(config.policies.len(), 5); // 4 default + 1 custom
    assert!(config.policies.iter().any(|p| p.name == "custom_policy"));
}

#[test]
fn test_retention_config_vacuum_enabled() {
    let config = RetentionConfig::default().vacuum_enabled(false);

    assert!(!config.vacuum_after_cleanup);
}

#[test]
fn test_retention_config_enabled() {
    let config = RetentionConfig::default().enabled(false);

    assert!(!config.enabled);
}

#[test]
fn test_retention_config_builder_chaining() {
    let config = RetentionConfig::default()
        .with_schedule("0 0 4 * * *")
        .add_policy(RetentionPolicy::new("custom", 15))
        .vacuum_enabled(false)
        .enabled(true);

    assert_eq!(config.schedule, "0 0 4 * * *");
    assert_eq!(config.policies.len(), 5);
    assert!(!config.vacuum_after_cleanup);
    assert!(config.enabled);
}

// ============================================================================
// RetentionConfig Clone and Debug Tests
// ============================================================================

#[test]
fn test_retention_config_clone() {
    let config = RetentionConfig::default().with_schedule("0 0 5 * * *");
    let cloned = config.clone();

    assert_eq!(config.enabled, cloned.enabled);
    assert_eq!(config.schedule, cloned.schedule);
    assert_eq!(config.vacuum_after_cleanup, cloned.vacuum_after_cleanup);
    assert_eq!(config.policies.len(), cloned.policies.len());
}

#[test]
fn test_retention_config_debug() {
    let config = RetentionConfig::default();
    let debug = format!("{:?}", config);

    assert!(debug.contains("RetentionConfig"));
    assert!(debug.contains("enabled"));
    assert!(debug.contains("schedule"));
}

// ============================================================================
// RetentionConfig Serialization Tests
// ============================================================================

#[test]
fn test_retention_config_serialize() {
    let config = RetentionConfig::default();
    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("\"enabled\":true"));
    assert!(json.contains("\"vacuum_after_cleanup\":true"));
    assert!(json.contains("\"schedule\""));
    assert!(json.contains("\"policies\""));
}

#[test]
fn test_retention_config_deserialize() {
    let json = r#"{
        "enabled": false,
        "schedule": "0 0 1 * * *",
        "policies": [],
        "vacuum_after_cleanup": false
    }"#;

    let config: RetentionConfig = serde_json::from_str(json).unwrap();

    assert!(!config.enabled);
    assert_eq!(config.schedule, "0 0 1 * * *");
    assert!(config.policies.is_empty());
    assert!(!config.vacuum_after_cleanup);
}

#[test]
fn test_retention_config_roundtrip() {
    let config = RetentionConfig::default()
        .with_schedule("0 0 6 * * *")
        .enabled(false);

    let json = serde_json::to_string(&config).unwrap();
    let parsed: RetentionConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.enabled, parsed.enabled);
    assert_eq!(config.schedule, parsed.schedule);
    assert_eq!(config.vacuum_after_cleanup, parsed.vacuum_after_cleanup);
    assert_eq!(config.policies.len(), parsed.policies.len());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_retention_policy_zero_retention_days() {
    let policy = RetentionPolicy::new("immediate_delete", 0);

    assert_eq!(policy.retention_days, 0);
}

#[test]
fn test_retention_policy_max_retention_days() {
    let policy = RetentionPolicy::new("forever", u32::MAX);

    assert_eq!(policy.retention_days, u32::MAX);
}

#[test]
fn test_retention_policy_empty_name() {
    let policy = RetentionPolicy::new("", 30);

    assert_eq!(policy.name, "");
}

#[test]
fn test_retention_policy_with_empty_module() {
    let policy = RetentionPolicy::new("test", 30).with_module("");

    assert_eq!(policy.module, Some("".to_string()));
}

#[test]
fn test_retention_config_empty_schedule() {
    let config = RetentionConfig::default().with_schedule("");

    assert_eq!(config.schedule, "");
}

#[test]
fn test_retention_config_multiple_policies_same_level() {
    let config = RetentionConfig::default()
        .add_policy(RetentionPolicy::new("error_api", 120).with_level(LogLevel::Error))
        .add_policy(RetentionPolicy::new("error_db", 180).with_level(LogLevel::Error));

    let error_policies: Vec<_> = config
        .policies
        .iter()
        .filter(|p| p.level == Some(LogLevel::Error))
        .collect();

    // Should have 3 error policies: default + 2 added
    assert_eq!(error_policies.len(), 3);
}
