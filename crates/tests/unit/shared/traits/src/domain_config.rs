//! Tests for domain_config: DomainConfigError, DomainConfigRegistry, and trait defaults.

use systemprompt_traits::domain_config::{DomainConfig, DomainConfigError, DomainConfigRegistry};
use systemprompt_traits::validation_report::ValidationReport;

// A DomainConfig that always succeeds, has default priority.
#[derive(Debug)]
struct AlwaysOk {
    id: &'static str,
}

impl DomainConfig for AlwaysOk {
    fn domain_id(&self) -> &'static str {
        self.id
    }
    fn load(&mut self, _config: &dyn systemprompt_traits::context::ConfigProvider) -> Result<(), DomainConfigError> {
        Ok(())
    }
    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        Ok(ValidationReport::default())
    }
}

// A DomainConfig with a custom priority.
#[derive(Debug)]
struct WithPriority {
    id: &'static str,
    priority: u32,
}

impl DomainConfig for WithPriority {
    fn domain_id(&self) -> &'static str {
        self.id
    }
    fn priority(&self) -> u32 {
        self.priority
    }
    fn load(&mut self, _config: &dyn systemprompt_traits::context::ConfigProvider) -> Result<(), DomainConfigError> {
        Ok(())
    }
    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        Ok(ValidationReport::default())
    }
}

// A DomainConfig that declares dependencies.
#[derive(Debug)]
struct WithDeps;

impl DomainConfig for WithDeps {
    fn domain_id(&self) -> &'static str {
        "dep-owner"
    }
    fn dependencies(&self) -> &[&'static str] {
        &["db", "auth"]
    }
    fn load(&mut self, _config: &dyn systemprompt_traits::context::ConfigProvider) -> Result<(), DomainConfigError> {
        Ok(())
    }
    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        Ok(ValidationReport::default())
    }
}

// --- DomainConfigError display ---

#[test]
fn load_error_display_contains_message() {
    let e = DomainConfigError::LoadError("missing key".to_owned());
    assert!(format!("{e}").contains("missing key"));
}

#[test]
fn not_found_display_contains_path() {
    let e = DomainConfigError::NotFound("/etc/foo.yaml".to_owned());
    assert!(format!("{e}").contains("/etc/foo.yaml"));
}

#[test]
fn parse_error_display_contains_detail() {
    let e = DomainConfigError::ParseError("invalid YAML".to_owned());
    assert!(format!("{e}").contains("invalid YAML"));
}

#[test]
fn validation_error_display_contains_detail() {
    let e = DomainConfigError::ValidationError("field required".to_owned());
    assert!(format!("{e}").contains("field required"));
}

#[test]
fn domain_config_errors_are_debug() {
    let variants: &[DomainConfigError] = &[
        DomainConfigError::LoadError("a".into()),
        DomainConfigError::NotFound("b".into()),
        DomainConfigError::ParseError("c".into()),
        DomainConfigError::ValidationError("d".into()),
    ];
    for e in variants {
        let s = format!("{e:?}");
        assert!(!s.is_empty());
    }
}

// --- DomainConfigRegistry construction ---

#[test]
fn new_registry_starts_empty() {
    let reg = DomainConfigRegistry::new();
    assert!(reg.validators_sorted().is_empty());
}

#[test]
fn default_registry_starts_empty() {
    let reg = DomainConfigRegistry::default();
    assert!(reg.validators_sorted().is_empty());
}

#[test]
fn register_adds_entry() {
    let mut reg = DomainConfigRegistry::new();
    reg.register(Box::new(AlwaysOk { id: "first" }));
    assert_eq!(reg.validators_sorted().len(), 1);
}

#[test]
fn register_multiple_entries() {
    let mut reg = DomainConfigRegistry::new();
    reg.register(Box::new(AlwaysOk { id: "a" }));
    reg.register(Box::new(AlwaysOk { id: "b" }));
    reg.register(Box::new(AlwaysOk { id: "c" }));
    assert_eq!(reg.validators_sorted().len(), 3);
}

// --- validators_sorted ordering ---

#[test]
fn validators_sorted_by_priority_ascending() {
    let mut reg = DomainConfigRegistry::new();
    reg.register(Box::new(WithPriority { id: "high", priority: 200 }));
    reg.register(Box::new(WithPriority { id: "low", priority: 10 }));
    reg.register(Box::new(WithPriority { id: "mid", priority: 50 }));

    let sorted = reg.validators_sorted();
    let ids: Vec<&str> = sorted.iter().map(|v| v.domain_id()).collect();
    assert_eq!(ids, vec!["low", "mid", "high"]);
}

#[test]
fn validators_sorted_is_stable_for_equal_priorities() {
    let mut reg = DomainConfigRegistry::new();
    reg.register(Box::new(WithPriority { id: "x", priority: 100 }));
    reg.register(Box::new(WithPriority { id: "y", priority: 100 }));
    let sorted = reg.validators_sorted();
    assert_eq!(sorted.len(), 2);
}

// --- default priority and dependencies ---

#[test]
fn default_priority_is_100() {
    let d = AlwaysOk { id: "test" };
    assert_eq!(d.priority(), 100);
}

#[test]
fn default_dependencies_is_empty_slice() {
    let d = AlwaysOk { id: "test" };
    assert!(d.dependencies().is_empty());
}

#[test]
fn custom_dependencies_are_returned() {
    let d = WithDeps;
    assert_eq!(d.dependencies(), &["db", "auth"]);
}

// --- validators_mut sorting ---

#[test]
fn validators_mut_visits_sorted_order() {
    let mut reg = DomainConfigRegistry::new();
    reg.register(Box::new(WithPriority { id: "z", priority: 300 }));
    reg.register(Box::new(WithPriority { id: "a", priority: 1 }));

    let ids: Vec<&str> = reg.validators_mut().map(|v| v.domain_id()).collect();
    assert_eq!(ids, vec!["a", "z"]);
}

// --- Debug impl ---

#[test]
fn registry_debug_mentions_validator_count() {
    let mut reg = DomainConfigRegistry::new();
    reg.register(Box::new(AlwaysOk { id: "one" }));
    let s = format!("{reg:?}");
    assert!(s.contains("1"));
}
