//! Tests for FilesConfigValidator domain validator behaviour.
//!
//! Exercises `DomainConfig::domain_id`, `priority`, and `validate` (which
//! short-circuits with an empty report when FilesConfig is not initialised
//! in this process — i.e. the typical unit-test environment).

use systemprompt_runtime::FilesConfigValidator;
use systemprompt_traits::DomainConfig;

#[test]
fn files_validator_default_is_constructible() {
    let v = FilesConfigValidator::default();
    let dbg = format!("{:?}", v);
    assert!(dbg.contains("FilesConfigValidator"));
}

#[test]
fn files_validator_new_matches_default() {
    let a = FilesConfigValidator::new();
    let b = FilesConfigValidator::default();
    assert_eq!(format!("{:?}", a), format!("{:?}", b));
}

#[test]
fn files_validator_clone_and_copy() {
    let a = FilesConfigValidator::new();
    let b = a;
    let c = a.clone();
    assert_eq!(format!("{:?}", a), format!("{:?}", b));
    assert_eq!(format!("{:?}", a), format!("{:?}", c));
}

#[test]
fn files_validator_domain_id() {
    let v = FilesConfigValidator::new();
    assert_eq!(DomainConfig::domain_id(&v), "files");
}

#[test]
fn files_validator_priority_constant() {
    let v = FilesConfigValidator::new();
    assert_eq!(DomainConfig::priority(&v), 5);
}

#[test]
fn files_validator_validate_returns_empty_report_without_init() {
    let v = FilesConfigValidator::new();
    let report = DomainConfig::validate(&v).expect("validate returns Ok when uninit");
    assert!(!report.has_errors());
}
