//! Unit tests for command requirements module
//!
//! Tests cover:
//! - CommandRequirements struct fields
//! - Predefined requirement constants (NONE, PROFILE_ONLY, etc.)
//! - HasRequirements trait

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::requirements::{CommandRequirements, HasRequirements};

// ============================================================================
// CommandRequirements Constant Tests
// ============================================================================

#[test]
fn test_requirements_none_all_false() {
    let reqs = CommandRequirements::NONE;
    assert!(!reqs.profile);
    assert!(!reqs.secrets);
    assert!(!reqs.paths);
    assert!(!reqs.database);
}

#[test]
fn test_requirements_profile_only() {
    let reqs = CommandRequirements::PROFILE_ONLY;
    assert!(reqs.profile);
    assert!(!reqs.secrets);
    assert!(!reqs.paths);
    assert!(!reqs.database);
}

#[test]
fn test_requirements_profile_and_secrets() {
    let reqs = CommandRequirements::PROFILE_AND_SECRETS;
    assert!(reqs.profile);
    assert!(reqs.secrets);
    assert!(!reqs.paths);
    assert!(!reqs.database);
}

#[test]
fn test_requirements_profile_secrets_and_paths() {
    let reqs = CommandRequirements::PROFILE_SECRETS_AND_PATHS;
    assert!(reqs.profile);
    assert!(reqs.secrets);
    assert!(reqs.paths);
    assert!(!reqs.database);
}

#[test]
fn test_requirements_full() {
    let reqs = CommandRequirements::FULL;
    assert!(reqs.profile);
    assert!(reqs.secrets);
    assert!(reqs.paths);
    assert!(reqs.database);
}

// ============================================================================
// CommandRequirements Default Tests
// ============================================================================

#[test]
fn test_requirements_default_all_false() {
    let reqs = CommandRequirements::default();
    assert!(!reqs.profile);
    assert!(!reqs.secrets);
    assert!(!reqs.paths);
    assert!(!reqs.database);
}

// ============================================================================
// CommandRequirements Debug Tests
// ============================================================================

#[test]
fn test_requirements_debug_format() {
    let reqs = CommandRequirements::FULL;
    let debug = format!("{:?}", reqs);
    assert!(debug.contains("CommandRequirements"));
    assert!(debug.contains("profile"));
    assert!(debug.contains("secrets"));
    assert!(debug.contains("paths"));
    assert!(debug.contains("database"));
}

// ============================================================================
// CommandRequirements Clone Tests
// ============================================================================

#[test]
fn test_requirements_clone() {
    let original = CommandRequirements::PROFILE_AND_SECRETS;
    let cloned = original;
    assert_eq!(cloned.profile, original.profile);
    assert_eq!(cloned.secrets, original.secrets);
    assert_eq!(cloned.paths, original.paths);
    assert_eq!(cloned.database, original.database);
}

// ============================================================================
// CommandRequirements Field Access Tests
// ============================================================================

#[test]
fn test_requirements_field_access_profile() {
    let reqs = CommandRequirements {
        profile: true,
        secrets: false,
        paths: false,
        database: false,
    };
    assert!(reqs.profile);
}

#[test]
fn test_requirements_field_access_secrets() {
    let reqs = CommandRequirements {
        profile: false,
        secrets: true,
        paths: false,
        database: false,
    };
    assert!(reqs.secrets);
}

#[test]
fn test_requirements_field_access_paths() {
    let reqs = CommandRequirements {
        profile: false,
        secrets: false,
        paths: true,
        database: false,
    };
    assert!(reqs.paths);
}

#[test]
fn test_requirements_field_access_database() {
    let reqs = CommandRequirements {
        profile: false,
        secrets: false,
        paths: false,
        database: true,
    };
    assert!(reqs.database);
}

// ============================================================================
// CommandRequirements Custom Combinations Tests
// ============================================================================

#[test]
fn test_requirements_custom_profile_and_database_only() {
    let reqs = CommandRequirements {
        profile: true,
        secrets: false,
        paths: false,
        database: true,
    };
    assert!(reqs.profile);
    assert!(!reqs.secrets);
    assert!(!reqs.paths);
    assert!(reqs.database);
}

#[test]
fn test_requirements_custom_all_except_database() {
    let reqs = CommandRequirements {
        profile: true,
        secrets: true,
        paths: true,
        database: false,
    };
    assert!(reqs.profile);
    assert!(reqs.secrets);
    assert!(reqs.paths);
    assert!(!reqs.database);
}

// ============================================================================
// HasRequirements Trait Tests
// ============================================================================

struct TestCommand {
    use_database: bool,
}

impl HasRequirements for TestCommand {
    fn requirements(&self) -> CommandRequirements {
        if self.use_database {
            CommandRequirements::FULL
        } else {
            CommandRequirements::PROFILE_ONLY
        }
    }
}

#[test]
fn test_has_requirements_trait_full() {
    let cmd = TestCommand { use_database: true };
    let reqs = cmd.requirements();
    assert!(reqs.database);
    assert!(reqs.profile);
}

#[test]
fn test_has_requirements_trait_profile_only() {
    let cmd = TestCommand { use_database: false };
    let reqs = cmd.requirements();
    assert!(!reqs.database);
    assert!(reqs.profile);
}

// ============================================================================
// Comparison Tests
// ============================================================================

#[test]
fn test_requirements_none_vs_default() {
    let none = CommandRequirements::NONE;
    let default = CommandRequirements::default();
    assert_eq!(none.profile, default.profile);
    assert_eq!(none.secrets, default.secrets);
    assert_eq!(none.paths, default.paths);
    assert_eq!(none.database, default.database);
}

#[test]
fn test_requirements_hierarchy_none_to_full() {
    let none = CommandRequirements::NONE;
    let profile_only = CommandRequirements::PROFILE_ONLY;
    let profile_and_secrets = CommandRequirements::PROFILE_AND_SECRETS;
    let profile_secrets_paths = CommandRequirements::PROFILE_SECRETS_AND_PATHS;
    let full = CommandRequirements::FULL;

    assert!(!none.profile);
    assert!(profile_only.profile && !profile_only.secrets);
    assert!(profile_and_secrets.profile && profile_and_secrets.secrets && !profile_and_secrets.paths);
    assert!(profile_secrets_paths.profile && profile_secrets_paths.secrets && profile_secrets_paths.paths && !profile_secrets_paths.database);
    assert!(full.profile && full.secrets && full.paths && full.database);
}
