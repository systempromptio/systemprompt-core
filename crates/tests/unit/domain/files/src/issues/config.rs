use systemprompt_files::{FilesConfigValidator, IssuesConfig};
use systemprompt_manifest::validators::DomainConfig;
use systemprompt_manifest::validators::validation_report::ValidationReport;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use systemprompt_traits::ConfigProvider;

struct StubProvider;

impl ConfigProvider for StubProvider {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }

    fn database_url(&self) -> &str {
        "postgres://unused"
    }

    fn system_path(&self) -> &str {
        "/unused"
    }

    fn api_port(&self) -> u16 {
        0
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn report(yaml: &str) -> ValidationReport {
    let bootstrap = ensure_test_bootstrap();
    std::fs::write(bootstrap.services_path.join("config/files.yaml"), yaml)
        .expect("write files.yaml");
    let mut validator = FilesConfigValidator::new(bootstrap.app_paths.clone());
    validator.load(&StubProvider).expect("load");
    validator.validate().expect("validate")
}

fn issue_errors(report: &ValidationReport) -> Vec<String> {
    report
        .errors
        .iter()
        .map(|issue| issue.field.clone())
        .filter(|field| field.starts_with("files.issues"))
        .collect()
}

fn issue_warnings(report: &ValidationReport) -> Vec<String> {
    report
        .warnings
        .iter()
        .map(|warning| warning.field.clone())
        .filter(|field| field.starts_with("files.issues"))
        .collect()
}

#[test]
fn issue_reports_are_off_by_default_with_admin_triage() {
    let config = IssuesConfig::default();
    assert!(!config.is_enabled());
    assert_eq!(config.triage_roles, vec!["admin".to_owned()]);
    assert!(config.max_reports_per_hour > 0);
    assert!(config.retention_days > 0);
}

#[test]
fn an_empty_section_deserialises_to_the_defaults() {
    let parsed: IssuesConfig = serde_yaml::from_str("{}").expect("parse");
    assert_eq!(parsed, IssuesConfig::default());
    let enabled: IssuesConfig = serde_yaml::from_str("enabled: true").expect("parse");
    assert!(enabled.is_enabled());
    assert_eq!(enabled.triage_roles, vec!["admin".to_owned()]);
}

#[test]
fn a_triager_holds_any_triage_role() {
    let config = IssuesConfig {
        triage_roles: vec!["admin".to_owned(), "support".to_owned()],
        ..IssuesConfig::default()
    };
    assert!(config.is_triager(&["user".to_owned(), "support".to_owned()]));
    assert!(config.is_triager(&["admin".to_owned()]));
    assert!(!config.is_triager(&["user".to_owned()]));
    assert!(!config.is_triager(&[]));
}

#[test]
fn a_disabled_section_is_not_validated() {
    let report = report("files:\n  issues:\n    retentionDays: 0\n    maxReportsPerHour: 0\n");
    assert!(issue_errors(&report).is_empty());
    assert!(issue_warnings(&report).is_empty());
}

#[test]
fn zero_bounds_and_no_triage_roles_are_errors_when_enabled() {
    let report = report(
        "files:\n  documents:\n    enabled: true\n  issues:\n    enabled: true\n    \
         retentionDays: 0\n    maxReportsPerHour: 0\n    triageRoles: []\n",
    );
    let mut errors = issue_errors(&report);
    errors.sort();
    assert_eq!(
        errors,
        vec![
            "files.issues.maxReportsPerHour",
            "files.issues.retentionDays",
            "files.issues.triageRoles",
        ]
    );
}

#[test]
fn attachments_without_the_document_store_are_a_warning() {
    let report = report("files:\n  issues:\n    enabled: true\n");
    assert!(issue_errors(&report).is_empty());
    assert_eq!(issue_warnings(&report), vec!["files.issues.maxAttachments"]);
}

#[test]
fn no_attachments_without_the_document_store_is_clean() {
    let report = report("files:\n  issues:\n    enabled: true\n    maxAttachments: 0\n");
    assert!(issue_errors(&report).is_empty());
    assert!(issue_warnings(&report).is_empty());
}

#[test]
fn attachments_with_the_document_store_are_clean() {
    let report = report("files:\n  documents:\n    enabled: true\n  issues:\n    enabled: true\n");
    assert!(issue_errors(&report).is_empty());
    assert!(issue_warnings(&report).is_empty());
}
