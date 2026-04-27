use std::path::PathBuf;
use systemprompt_traits::validation_report::{
    StartupValidationError, StartupValidationReport, ValidationError, ValidationReport,
    ValidationWarning,
};
use systemprompt_traits::{DomainConfig, DomainConfigError, DomainConfigRegistry};

#[test]
fn test_validation_error_display_basic() {
    let error = ValidationError::new("database_url", "Connection refused");
    let display = format!("{}", error);
    assert!(display.contains("database_url"));
    assert!(display.contains("Connection refused"));
}

#[test]
fn test_validation_error_display_with_path() {
    let error = ValidationError::new("config", "File not found")
        .with_path(PathBuf::from("/etc/app/config.yaml"));
    let display = format!("{}", error);
    assert!(display.contains("Path:"));
    assert!(display.contains("/etc/app/config.yaml"));
}

#[test]
fn test_validation_error_display_with_suggestion() {
    let error = ValidationError::new("field", "message").with_suggestion("Run setup first");
    let display = format!("{}", error);
    assert!(display.contains("To fix:"));
    assert!(display.contains("Run setup first"));
}

#[test]
fn test_validation_error_display_full_chain() {
    let error = ValidationError::new("database_url", "Connection refused")
        .with_path("/config/db.yaml")
        .with_suggestion("Check credentials");
    let display = format!("{}", error);
    assert!(display.contains("database_url"));
    assert!(display.contains("Connection refused"));
    assert!(display.contains("Path:"));
    assert!(display.contains("To fix:"));
}

#[test]
fn test_validation_report_is_clean_empty() {
    let report = ValidationReport::new("test");
    assert!(report.is_clean());
}

#[test]
fn test_validation_report_is_clean_with_error() {
    let mut report = ValidationReport::new("test");
    report.add_error(ValidationError::new("f", "m"));
    assert!(!report.is_clean());
}

#[test]
fn test_validation_report_is_clean_with_warning() {
    let mut report = ValidationReport::new("test");
    report.add_warning(ValidationWarning::new("f", "m"));
    assert!(!report.is_clean());
}

#[test]
fn test_validation_report_merge_errors() {
    let mut report_a = ValidationReport::new("domain_a");
    report_a.add_error(ValidationError::new("field_a", "error_a"));

    let mut report_b = ValidationReport::new("domain_b");
    report_b.add_error(ValidationError::new("field_b", "error_b"));

    report_a.merge(report_b);
    assert_eq!(report_a.errors.len(), 2);
    assert_eq!(report_a.errors[0].field, "field_a");
    assert_eq!(report_a.errors[1].field, "field_b");
}

#[test]
fn test_validation_report_merge_warnings() {
    let mut report_a = ValidationReport::new("domain");
    report_a.add_warning(ValidationWarning::new("w1", "msg1"));

    let mut report_b = ValidationReport::new("other");
    report_b.add_warning(ValidationWarning::new("w2", "msg2"));

    report_a.merge(report_b);
    assert_eq!(report_a.warnings.len(), 2);
}

#[test]
fn test_validation_report_merge_preserves_domain() {
    let mut report_a = ValidationReport::new("original_domain");
    let report_b = ValidationReport::new("other_domain");
    report_a.merge(report_b);
    assert_eq!(report_a.domain, "original_domain");
}

#[test]
fn test_startup_report_display_zero_counts() {
    let report = StartupValidationReport::new();
    let display = format!("{}", report);
    assert!(display.contains("0 error(s)"));
    assert!(display.contains("0 warning(s)"));
}

#[test]
fn test_startup_report_display_with_errors() {
    let mut report = StartupValidationReport::new();
    let mut domain = ValidationReport::new("test");
    domain.add_error(ValidationError::new("f1", "m1"));
    domain.add_error(ValidationError::new("f2", "m2"));
    report.add_domain(domain);
    let display = format!("{}", report);
    assert!(display.contains("2 error(s)"));
}

#[test]
fn test_startup_report_add_extension() {
    let mut report = StartupValidationReport::new();
    let ext = ValidationReport::new("ext:my-extension");
    report.add_extension(ext);
    assert_eq!(report.extensions.len(), 1);
    assert_eq!(report.extensions[0].domain, "ext:my-extension");
}

#[test]
fn test_startup_report_error_count_includes_extensions() {
    let mut report = StartupValidationReport::new();

    let mut domain = ValidationReport::new("web");
    domain.add_error(ValidationError::new("f1", "m1"));
    report.add_domain(domain);

    let mut ext = ValidationReport::new("ext:plugin");
    ext.add_error(ValidationError::new("f2", "m2"));
    ext.add_error(ValidationError::new("f3", "m3"));
    report.add_extension(ext);

    assert_eq!(report.error_count(), 3);
}

#[test]
fn test_startup_report_warning_count_includes_extensions() {
    let mut report = StartupValidationReport::new();

    let mut domain = ValidationReport::new("web");
    domain.add_warning(ValidationWarning::new("w1", "m1"));
    report.add_domain(domain);

    let mut ext = ValidationReport::new("ext:plugin");
    ext.add_warning(ValidationWarning::new("w2", "m2"));
    report.add_extension(ext);

    assert_eq!(report.warning_count(), 2);
}

#[test]
fn test_startup_report_has_errors_from_extensions() {
    let mut report = StartupValidationReport::new();
    let mut ext = ValidationReport::new("ext:broken");
    ext.add_error(ValidationError::new("config", "missing"));
    report.add_extension(ext);
    assert!(report.has_errors());
}

#[test]
fn test_startup_report_has_warnings_from_domain() {
    let mut report = StartupValidationReport::new();
    let mut domain = ValidationReport::new("files");
    domain.add_warning(ValidationWarning::new("storage", "not optimal"));
    report.add_domain(domain);
    assert!(report.has_warnings());
}

#[test]
fn test_startup_validation_error_from_report() {
    let mut report = StartupValidationReport::new();
    let mut domain = ValidationReport::new("test");
    domain.add_error(ValidationError::new("field", "msg"));
    report.add_domain(domain);

    let error: StartupValidationError = report.into();
    let display = format!("{}", error);
    assert!(display.contains("Startup validation failed"));
}

#[test]
fn test_domain_config_error_load_error() {
    let error = DomainConfigError::LoadError("connection timeout".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Failed to load config"));
    assert!(display.contains("connection timeout"));
}

#[test]
fn test_domain_config_error_not_found() {
    let error = DomainConfigError::NotFound("/path/to/config.yaml".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Config file not found"));
}

#[test]
fn test_domain_config_error_parse_error() {
    let error = DomainConfigError::ParseError("invalid YAML at line 5".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Failed to parse config"));
}

#[test]
fn test_domain_config_error_validation_error() {
    let error = DomainConfigError::ValidationError("missing required field".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Validation failed"));
}

#[test]
fn test_domain_config_registry_new_is_empty() {
    let registry = DomainConfigRegistry::new();
    assert!(registry.validators_sorted().is_empty());
}

#[test]
fn test_domain_config_registry_default_is_empty() {
    let registry = DomainConfigRegistry::default();
    assert!(registry.validators_sorted().is_empty());
}

#[test]
fn test_domain_config_registry_debug() {
    let registry = DomainConfigRegistry::new();
    let debug_str = format!("{:?}", registry);
    assert!(debug_str.contains("DomainConfigRegistry"));
    assert!(debug_str.contains("validator_count"));
}

#[derive(Debug)]
struct TestValidator {
    id: &'static str,
    prio: u32,
}

impl DomainConfig for TestValidator {
    fn domain_id(&self) -> &'static str {
        self.id
    }

    fn priority(&self) -> u32 {
        self.prio
    }

    fn load(
        &mut self,
        _config: &dyn systemprompt_traits::ConfigProvider,
    ) -> Result<(), DomainConfigError> {
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        Ok(ValidationReport::new(self.id))
    }
}

#[test]
fn test_domain_config_registry_register_and_count() {
    let mut registry = DomainConfigRegistry::new();
    registry.register(Box::new(TestValidator {
        id: "test",
        prio: 10,
    }));
    assert_eq!(registry.validators_sorted().len(), 1);
}

#[test]
fn test_domain_config_registry_sorted_by_priority() {
    let mut registry = DomainConfigRegistry::new();
    registry.register(Box::new(TestValidator {
        id: "low_priority",
        prio: 100,
    }));
    registry.register(Box::new(TestValidator {
        id: "high_priority",
        prio: 1,
    }));
    registry.register(Box::new(TestValidator {
        id: "medium_priority",
        prio: 50,
    }));

    let sorted = registry.validators_sorted();
    assert_eq!(sorted[0].domain_id(), "high_priority");
    assert_eq!(sorted[1].domain_id(), "medium_priority");
    assert_eq!(sorted[2].domain_id(), "low_priority");
}

#[test]
fn test_domain_config_default_dependencies_empty() {
    let validator = TestValidator {
        id: "test",
        prio: 10,
    };
    assert!(validator.dependencies().is_empty());
}

#[test]
fn test_validation_warning_with_suggestion_value() {
    let warning = ValidationWarning::new("geoip", "Database not configured")
        .with_suggestion("Download MaxMind database");
    assert_eq!(warning.suggestion.unwrap(), "Download MaxMind database");
}
