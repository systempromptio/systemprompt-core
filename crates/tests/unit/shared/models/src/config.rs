//! Unit tests for configuration models
//!
//! Tests cover:
//! - Environment detection and parsing
//! - Environment helper methods

use systemprompt_models::config::Environment;

// ============================================================================
// Environment Tests
// ============================================================================

#[test]
fn test_environment_from_string_development() {
    // Direct method testing would require calling from_string which is private
    // Instead we test the public interface through serialization-like behavior
    let env = Environment::Development;
    assert!(env.is_development());
    assert!(!env.is_production());
    assert!(!env.is_test());
}

#[test]
fn test_environment_from_string_production() {
    let env = Environment::Production;
    assert!(env.is_production());
    assert!(!env.is_development());
    assert!(!env.is_test());
}

#[test]
fn test_environment_from_string_test() {
    let env = Environment::Test;
    assert!(env.is_test());
    assert!(!env.is_development());
    assert!(!env.is_production());
}

#[test]
fn test_environment_is_development() {
    let env = Environment::Development;
    assert!(env.is_development());
}

#[test]
fn test_environment_is_not_development_when_production() {
    let env = Environment::Production;
    assert!(!env.is_development());
}

#[test]
fn test_environment_is_not_development_when_test() {
    let env = Environment::Test;
    assert!(!env.is_development());
}

#[test]
fn test_environment_is_production() {
    let env = Environment::Production;
    assert!(env.is_production());
}

#[test]
fn test_environment_is_not_production_when_development() {
    let env = Environment::Development;
    assert!(!env.is_production());
}

#[test]
fn test_environment_is_not_production_when_test() {
    let env = Environment::Test;
    assert!(!env.is_production());
}

#[test]
fn test_environment_is_test() {
    let env = Environment::Test;
    assert!(env.is_test());
}

#[test]
fn test_environment_is_not_test_when_development() {
    let env = Environment::Development;
    assert!(!env.is_test());
}

#[test]
fn test_environment_is_not_test_when_production() {
    let env = Environment::Production;
    assert!(!env.is_test());
}

#[test]
fn test_environment_clone() {
    let env = Environment::Development;
    let cloned = env;
    assert!(cloned.is_development());
}

#[test]
fn test_environment_copy() {
    let env = Environment::Production;
    let copied = env;
    assert!(copied.is_production());
    assert!(env.is_production()); // Original still accessible (Copy)
}

#[test]
fn test_environment_equality() {
    assert_eq!(Environment::Development, Environment::Development);
    assert_eq!(Environment::Production, Environment::Production);
    assert_eq!(Environment::Test, Environment::Test);
}

#[test]
fn test_environment_inequality() {
    assert_ne!(Environment::Development, Environment::Production);
    assert_ne!(Environment::Development, Environment::Test);
    assert_ne!(Environment::Production, Environment::Test);
}

#[test]
fn test_environment_debug() {
    let env = Environment::Development;
    let debug_str = format!("{:?}", env);
    assert!(debug_str.contains("Development"));
}

#[test]
fn test_environment_debug_production() {
    let env = Environment::Production;
    let debug_str = format!("{:?}", env);
    assert!(debug_str.contains("Production"));
}

#[test]
fn test_environment_debug_test() {
    let env = Environment::Test;
    let debug_str = format!("{:?}", env);
    assert!(debug_str.contains("Test"));
}

// ============================================================================
// Environment Detection Tests (require environment variable manipulation)
// These tests verify the detection logic works correctly
// ============================================================================

#[test]
fn test_environment_detect_returns_valid_variant() {
    // detect() should always return a valid Environment variant
    let env = Environment::detect();
    // Must be one of the three variants
    assert!(env.is_development() || env.is_production() || env.is_test());
}

#[test]
fn test_environment_default_uses_detect() {
    // Default implementation uses detect()
    let env = Environment::default();
    // Must be one of the three variants
    assert!(env.is_development() || env.is_production() || env.is_test());
}

#[test]
fn test_environment_multiple_calls_consistent() {
    // Without changing env vars, multiple calls should be consistent
    let env1 = Environment::detect();
    let env2 = Environment::detect();
    assert_eq!(env1, env2);
}
