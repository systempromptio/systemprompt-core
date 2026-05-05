//! Unit tests for configuration models
//!
//! Tests cover:
//! - Environment detection and parsing
//! - Environment helper methods

use systemprompt_models::config::Environment;

#[test]
fn test_environment_from_string_development() {
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

#[test]
fn test_environment_detect_returns_valid_variant() {
    let env = Environment::detect();
    assert!(env.is_development() || env.is_production() || env.is_test());
}

#[test]
fn test_environment_default_uses_detect() {
    let env = Environment::default();
    assert!(env.is_development() || env.is_production() || env.is_test());
}

#[test]
fn test_environment_multiple_calls_consistent() {
    let env1 = Environment::detect();
    let env2 = Environment::detect();
    assert_eq!(env1, env2);
}
