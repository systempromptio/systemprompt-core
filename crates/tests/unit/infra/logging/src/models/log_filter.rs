//! Unit tests for LogFilter struct

use systemprompt_core_logging::LogFilter;

// ============================================================================
// LogFilter Creation Tests
// ============================================================================

#[test]
fn test_log_filter_new() {
    let filter = LogFilter::new(1, 10);

    assert_eq!(filter.page(), 1);
    assert_eq!(filter.per_page(), 10);
    assert!(filter.level().is_none());
    assert!(filter.module().is_none());
    assert!(filter.message().is_none());
}

#[test]
fn test_log_filter_new_different_pagination() {
    let filter = LogFilter::new(5, 50);

    assert_eq!(filter.page(), 5);
    assert_eq!(filter.per_page(), 50);
}

#[test]
fn test_log_filter_new_boundary_values() {
    let filter = LogFilter::new(0, 0);
    assert_eq!(filter.page(), 0);
    assert_eq!(filter.per_page(), 0);

    let filter = LogFilter::new(i32::MAX, i32::MAX);
    assert_eq!(filter.page(), i32::MAX);
    assert_eq!(filter.per_page(), i32::MAX);
}

// ============================================================================
// LogFilter Default Tests
// ============================================================================

#[test]
fn test_log_filter_default() {
    let filter = LogFilter::default();

    assert_eq!(filter.page(), 0);
    assert_eq!(filter.per_page(), 0);
    assert!(filter.level().is_none());
    assert!(filter.module().is_none());
    assert!(filter.message().is_none());
}

// ============================================================================
// LogFilter Builder Pattern Tests
// ============================================================================

#[test]
fn test_log_filter_with_level() {
    let filter = LogFilter::new(1, 10).with_level("ERROR");

    assert_eq!(filter.level(), Some("ERROR"));
}

#[test]
fn test_log_filter_with_level_string() {
    let filter = LogFilter::new(1, 10).with_level(String::from("WARN"));

    assert_eq!(filter.level(), Some("WARN"));
}

#[test]
fn test_log_filter_with_module() {
    let filter = LogFilter::new(1, 10).with_module("auth");

    assert_eq!(filter.module(), Some("auth"));
}

#[test]
fn test_log_filter_with_module_string() {
    let filter = LogFilter::new(1, 10).with_module(String::from("auth::login"));

    assert_eq!(filter.module(), Some("auth::login"));
}

#[test]
fn test_log_filter_with_message() {
    let filter = LogFilter::new(1, 10).with_message("failed");

    assert_eq!(filter.message(), Some("failed"));
}

#[test]
fn test_log_filter_with_message_string() {
    let filter = LogFilter::new(1, 10).with_message(String::from("authentication failed"));

    assert_eq!(filter.message(), Some("authentication failed"));
}

#[test]
fn test_log_filter_builder_chaining() {
    let filter = LogFilter::new(2, 25)
        .with_level("ERROR")
        .with_module("database")
        .with_message("connection");

    assert_eq!(filter.page(), 2);
    assert_eq!(filter.per_page(), 25);
    assert_eq!(filter.level(), Some("ERROR"));
    assert_eq!(filter.module(), Some("database"));
    assert_eq!(filter.message(), Some("connection"));
}

#[test]
fn test_log_filter_builder_partial_chain() {
    let filter = LogFilter::new(1, 10).with_level("INFO").with_module("api");

    assert_eq!(filter.level(), Some("INFO"));
    assert_eq!(filter.module(), Some("api"));
    assert!(filter.message().is_none());
}

// ============================================================================
// LogFilter Accessor Tests
// ============================================================================

#[test]
fn test_log_filter_page_accessor() {
    let filter = LogFilter::new(42, 10);
    assert_eq!(filter.page(), 42);
}

#[test]
fn test_log_filter_per_page_accessor() {
    let filter = LogFilter::new(1, 100);
    assert_eq!(filter.per_page(), 100);
}

#[test]
fn test_log_filter_level_accessor_none() {
    let filter = LogFilter::new(1, 10);
    assert!(filter.level().is_none());
}

#[test]
fn test_log_filter_level_accessor_some() {
    let filter = LogFilter::new(1, 10).with_level("DEBUG");
    assert_eq!(filter.level(), Some("DEBUG"));
}

#[test]
fn test_log_filter_module_accessor_none() {
    let filter = LogFilter::new(1, 10);
    assert!(filter.module().is_none());
}

#[test]
fn test_log_filter_module_accessor_some() {
    let filter = LogFilter::new(1, 10).with_module("test::module");
    assert_eq!(filter.module(), Some("test::module"));
}

#[test]
fn test_log_filter_message_accessor_none() {
    let filter = LogFilter::new(1, 10);
    assert!(filter.message().is_none());
}

#[test]
fn test_log_filter_message_accessor_some() {
    let filter = LogFilter::new(1, 10).with_message("error occurred");
    assert_eq!(filter.message(), Some("error occurred"));
}

// ============================================================================
// LogFilter Clone and Debug Tests
// ============================================================================

#[test]
fn test_log_filter_clone() {
    let filter = LogFilter::new(1, 10)
        .with_level("ERROR")
        .with_module("test")
        .with_message("msg");
    let cloned = filter.clone();

    assert_eq!(filter.page(), cloned.page());
    assert_eq!(filter.per_page(), cloned.per_page());
    assert_eq!(filter.level(), cloned.level());
    assert_eq!(filter.module(), cloned.module());
    assert_eq!(filter.message(), cloned.message());
}

#[test]
fn test_log_filter_debug() {
    let filter = LogFilter::new(1, 10).with_level("INFO");
    let debug = format!("{:?}", filter);

    assert!(debug.contains("LogFilter"));
}

// ============================================================================
// LogFilter Edge Cases
// ============================================================================

#[test]
fn test_log_filter_with_empty_level() {
    let filter = LogFilter::new(1, 10).with_level("");
    assert_eq!(filter.level(), Some(""));
}

#[test]
fn test_log_filter_with_empty_module() {
    let filter = LogFilter::new(1, 10).with_module("");
    assert_eq!(filter.module(), Some(""));
}

#[test]
fn test_log_filter_with_empty_message() {
    let filter = LogFilter::new(1, 10).with_message("");
    assert_eq!(filter.message(), Some(""));
}

#[test]
fn test_log_filter_with_special_characters() {
    let filter = LogFilter::new(1, 10)
        .with_level("ERROR")
        .with_module("auth::oauth2")
        .with_message("user@example.com failed to login");

    assert_eq!(filter.level(), Some("ERROR"));
    assert_eq!(filter.module(), Some("auth::oauth2"));
    assert_eq!(filter.message(), Some("user@example.com failed to login"));
}

#[test]
fn test_log_filter_with_unicode() {
    let filter = LogFilter::new(1, 10)
        .with_module("i18n")
        .with_message("Failed to process text");

    assert_eq!(filter.module(), Some("i18n"));
    assert_eq!(filter.message(), Some("Failed to process text"));
}

#[test]
fn test_log_filter_negative_pagination() {
    let filter = LogFilter::new(-1, -10);

    assert_eq!(filter.page(), -1);
    assert_eq!(filter.per_page(), -10);
}

#[test]
fn test_log_filter_with_long_strings() {
    let long_module = "a".repeat(1000);
    let long_message = "b".repeat(1000);

    let filter = LogFilter::new(1, 10)
        .with_module(long_module.clone())
        .with_message(long_message.clone());

    assert_eq!(filter.module(), Some(long_module.as_str()));
    assert_eq!(filter.message(), Some(long_message.as_str()));
}
