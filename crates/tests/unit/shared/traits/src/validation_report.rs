//! Tests for validation_report module types.

use std::path::PathBuf;
use systemprompt_traits::{
    StartupValidationError, StartupValidationReport, ValidationReport, ValidationWarning,
};

// Note: ValidationError from validation_report module has different fields than
// the one from validation module (path, suggestion vs context).
// We test the validation_report::ValidationError here.
use systemprompt_traits::validation_report::ValidationError;

mod validation_error_tests {
    use super::*;

    #[test]
    fn new_creates_error() {
        let err = ValidationError::new("config.port", "Invalid port number");

        assert_eq!(err.field, "config.port");
        assert_eq!(err.message, "Invalid port number");
        assert!(err.path.is_none());
        assert!(err.suggestion.is_none());
    }

    #[test]
    fn with_path_adds_path() {
        let err = ValidationError::new("database", "Connection failed")
            .with_path("/etc/config.yaml");

        assert!(err.path.is_some());
        assert_eq!(err.path.unwrap(), PathBuf::from("/etc/config.yaml"));
    }

    #[test]
    fn with_suggestion_adds_suggestion() {
        let err = ValidationError::new("api_key", "Missing API key")
            .with_suggestion("Set the API_KEY environment variable");

        assert!(err.suggestion.is_some());
        assert_eq!(err.suggestion.unwrap(), "Set the API_KEY environment variable");
    }

    #[test]
    fn builders_are_chainable() {
        let err = ValidationError::new("field", "message")
            .with_path("/path/to/file")
            .with_suggestion("suggestion text");

        assert_eq!(err.field, "field");
        assert_eq!(err.message, "message");
        assert!(err.path.is_some());
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn display_basic_error() {
        let err = ValidationError::new("setting", "Invalid value");
        let display = format!("{}", err);

        assert!(display.contains("setting"));
        assert!(display.contains("Invalid value"));
    }

    #[test]
    fn display_with_path() {
        let err = ValidationError::new("config", "Parse error")
            .with_path("/etc/app/config.yaml");
        let display = format!("{}", err);

        assert!(display.contains("Path:"));
        assert!(display.contains("config.yaml"));
    }

    #[test]
    fn display_with_suggestion() {
        let err = ValidationError::new("port", "Port in use")
            .with_suggestion("Try a different port");
        let display = format!("{}", err);

        assert!(display.contains("To fix:"));
        assert!(display.contains("Try a different port"));
    }
}

mod validation_warning_tests {
    use super::*;

    #[test]
    fn new_creates_warning() {
        let warning = ValidationWarning::new("deprecated_option", "This option is deprecated");

        assert_eq!(warning.field, "deprecated_option");
        assert_eq!(warning.message, "This option is deprecated");
        assert!(warning.suggestion.is_none());
    }

    #[test]
    fn with_suggestion_adds_suggestion() {
        let warning = ValidationWarning::new("old_format", "Format is outdated")
            .with_suggestion("Use the new YAML format");

        assert!(warning.suggestion.is_some());
        assert_eq!(warning.suggestion.unwrap(), "Use the new YAML format");
    }

    #[test]
    fn warning_is_clone() {
        let warning = ValidationWarning::new("test", "message")
            .with_suggestion("fix");
        let cloned = warning.clone();

        assert_eq!(warning.field, cloned.field);
        assert_eq!(warning.message, cloned.message);
        assert_eq!(warning.suggestion, cloned.suggestion);
    }
}

mod validation_report_tests {
    use super::*;

    #[test]
    fn new_creates_empty_report() {
        let report = ValidationReport::new("test_domain");

        assert_eq!(report.domain, "test_domain");
        assert!(report.errors.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn add_error_appends_error() {
        let mut report = ValidationReport::new("domain");
        report.add_error(ValidationError::new("field1", "error1"));
        report.add_error(ValidationError::new("field2", "error2"));

        assert_eq!(report.errors.len(), 2);
    }

    #[test]
    fn add_warning_appends_warning() {
        let mut report = ValidationReport::new("domain");
        report.add_warning(ValidationWarning::new("warn1", "message1"));

        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn has_errors_returns_true_when_errors_exist() {
        let mut report = ValidationReport::new("domain");
        assert!(!report.has_errors());

        report.add_error(ValidationError::new("field", "error"));
        assert!(report.has_errors());
    }

    #[test]
    fn has_warnings_returns_true_when_warnings_exist() {
        let mut report = ValidationReport::new("domain");
        assert!(!report.has_warnings());

        report.add_warning(ValidationWarning::new("field", "warning"));
        assert!(report.has_warnings());
    }

    #[test]
    fn is_clean_returns_true_when_no_issues() {
        let report = ValidationReport::new("clean_domain");
        assert!(report.is_clean());
    }

    #[test]
    fn is_clean_returns_false_with_errors() {
        let mut report = ValidationReport::new("domain");
        report.add_error(ValidationError::new("field", "error"));
        assert!(!report.is_clean());
    }

    #[test]
    fn is_clean_returns_false_with_warnings() {
        let mut report = ValidationReport::new("domain");
        report.add_warning(ValidationWarning::new("field", "warning"));
        assert!(!report.is_clean());
    }

    #[test]
    fn merge_combines_reports() {
        let mut report1 = ValidationReport::new("domain1");
        report1.add_error(ValidationError::new("f1", "e1"));
        report1.add_warning(ValidationWarning::new("w1", "m1"));

        let mut report2 = ValidationReport::new("domain2");
        report2.add_error(ValidationError::new("f2", "e2"));
        report2.add_warning(ValidationWarning::new("w2", "m2"));

        report1.merge(report2);

        assert_eq!(report1.errors.len(), 2);
        assert_eq!(report1.warnings.len(), 2);
    }

    #[test]
    fn default_creates_empty_report() {
        let report = ValidationReport::default();
        assert!(report.domain.is_empty());
        assert!(report.errors.is_empty());
        assert!(report.warnings.is_empty());
    }
}

mod startup_validation_report_tests {
    use super::*;

    #[test]
    fn new_creates_empty_report() {
        let report = StartupValidationReport::new();

        assert!(report.profile_path.is_none());
        assert!(report.domains.is_empty());
        assert!(report.extensions.is_empty());
    }

    #[test]
    fn with_profile_path_sets_path() {
        let report = StartupValidationReport::new()
            .with_profile_path("/etc/profile.yaml");

        assert!(report.profile_path.is_some());
        assert_eq!(report.profile_path.unwrap(), PathBuf::from("/etc/profile.yaml"));
    }

    #[test]
    fn add_domain_appends_domain_report() {
        let mut report = StartupValidationReport::new();
        report.add_domain(ValidationReport::new("paths"));
        report.add_domain(ValidationReport::new("web"));

        assert_eq!(report.domains.len(), 2);
    }

    #[test]
    fn add_extension_appends_extension_report() {
        let mut report = StartupValidationReport::new();
        report.add_extension(ValidationReport::new("ext1"));

        assert_eq!(report.extensions.len(), 1);
    }

    #[test]
    fn has_errors_checks_domains() {
        let mut report = StartupValidationReport::new();
        assert!(!report.has_errors());

        let mut domain = ValidationReport::new("test");
        domain.add_error(ValidationError::new("field", "error"));
        report.add_domain(domain);

        assert!(report.has_errors());
    }

    #[test]
    fn has_errors_checks_extensions() {
        let mut report = StartupValidationReport::new();

        let mut ext = ValidationReport::new("ext");
        ext.add_error(ValidationError::new("field", "error"));
        report.add_extension(ext);

        assert!(report.has_errors());
    }

    #[test]
    fn has_warnings_checks_domains() {
        let mut report = StartupValidationReport::new();
        assert!(!report.has_warnings());

        let mut domain = ValidationReport::new("test");
        domain.add_warning(ValidationWarning::new("field", "warning"));
        report.add_domain(domain);

        assert!(report.has_warnings());
    }

    #[test]
    fn has_warnings_checks_extensions() {
        let mut report = StartupValidationReport::new();

        let mut ext = ValidationReport::new("ext");
        ext.add_warning(ValidationWarning::new("field", "warning"));
        report.add_extension(ext);

        assert!(report.has_warnings());
    }

    #[test]
    fn error_count_sums_all_errors() {
        let mut report = StartupValidationReport::new();

        let mut domain1 = ValidationReport::new("d1");
        domain1.add_error(ValidationError::new("f1", "e1"));
        domain1.add_error(ValidationError::new("f2", "e2"));

        let mut domain2 = ValidationReport::new("d2");
        domain2.add_error(ValidationError::new("f3", "e3"));

        let mut ext = ValidationReport::new("ext");
        ext.add_error(ValidationError::new("f4", "e4"));

        report.add_domain(domain1);
        report.add_domain(domain2);
        report.add_extension(ext);

        assert_eq!(report.error_count(), 4);
    }

    #[test]
    fn warning_count_sums_all_warnings() {
        let mut report = StartupValidationReport::new();

        let mut domain = ValidationReport::new("d1");
        domain.add_warning(ValidationWarning::new("w1", "m1"));
        domain.add_warning(ValidationWarning::new("w2", "m2"));

        let mut ext = ValidationReport::new("ext");
        ext.add_warning(ValidationWarning::new("w3", "m3"));

        report.add_domain(domain);
        report.add_extension(ext);

        assert_eq!(report.warning_count(), 3);
    }

    #[test]
    fn display_shows_counts() {
        let mut report = StartupValidationReport::new();

        let mut domain = ValidationReport::new("test");
        domain.add_error(ValidationError::new("f1", "e1"));
        domain.add_error(ValidationError::new("f2", "e2"));
        domain.add_warning(ValidationWarning::new("w1", "m1"));
        report.add_domain(domain);

        let display = format!("{}", report);
        assert!(display.contains("2 error(s)"));
        assert!(display.contains("1 warning(s)"));
    }

    #[test]
    fn default_creates_empty_report() {
        let report = StartupValidationReport::default();
        assert!(report.profile_path.is_none());
        assert!(report.domains.is_empty());
        assert!(report.extensions.is_empty());
    }
}

mod startup_validation_error_tests {
    use super::*;

    #[test]
    fn from_report_creates_error() {
        let mut report = StartupValidationReport::new();
        let mut domain = ValidationReport::new("test");
        domain.add_error(ValidationError::new("field", "error"));
        report.add_domain(domain);

        let error: StartupValidationError = report.into();
        assert_eq!(error.0.error_count(), 1);
    }

    #[test]
    fn error_display_shows_report_info() {
        let mut report = StartupValidationReport::new();
        let mut domain = ValidationReport::new("test");
        domain.add_error(ValidationError::new("f1", "e1"));
        domain.add_warning(ValidationWarning::new("w1", "m1"));
        report.add_domain(domain);

        let error = StartupValidationError(report);
        let display = format!("{}", error);

        assert!(display.contains("Startup validation failed"));
        assert!(display.contains("1 error(s)"));
        assert!(display.contains("1 warning(s)"));
    }

    #[test]
    fn error_is_std_error() {
        let report = StartupValidationReport::new();
        let error = StartupValidationError(report);
        let _: &dyn std::error::Error = &error;
    }
}
