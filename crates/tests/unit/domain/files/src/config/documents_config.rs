//! `files.documents` defaults and validator bounds. Each test runs in its own
//! nextest process and writes its own `files.yaml`.

use systemprompt_files::{
    DocumentLimits, DocumentsConfig, FilesConfigValidator, MAX_DOCUMENT_UPLOAD_BYTES,
    MIN_DOCUMENT_UPLOAD_BYTES,
};
use systemprompt_manifest::validators::DomainConfig;
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

fn error_fields(yaml: &str) -> Vec<String> {
    let bootstrap = ensure_test_bootstrap();
    std::fs::write(bootstrap.services_path.join("config/files.yaml"), yaml)
        .expect("write files.yaml");
    let mut validator = FilesConfigValidator::new(bootstrap.app_paths.clone());
    validator.load(&StubProvider).expect("load");
    validator
        .validate()
        .expect("validate")
        .errors
        .into_iter()
        .map(|issue| issue.field)
        .collect()
}

#[test]
fn the_store_is_disabled_by_default_and_enabled_by_the_switch() {
    assert!(!DocumentsConfig::default().is_enabled());
    let absent: DocumentsConfig = serde_yaml::from_str("maxUploadBytes: 2097152\n").expect("parse");
    assert!(!absent.is_enabled());
    let enabled: DocumentsConfig = serde_yaml::from_str("enabled: true\n").expect("parse");
    assert!(enabled.is_enabled());
    assert_eq!(
        enabled.max_upload_bytes,
        DocumentsConfig::default().max_upload_bytes
    );
}

#[test]
fn defaults_apply_when_section_is_absent_or_partial() {
    let config = DocumentsConfig::default();
    assert_eq!(config.max_upload_bytes, 64 * 1024 * 1024);
    assert_eq!(config.blob_gc_grace_hours, 24);
    assert_eq!(config.presence_ttl_secs, 90);
    let partial: DocumentsConfig = serde_yaml::from_str("blobGcGraceHours: 48\n").expect("parse");
    assert_eq!(partial.blob_gc_grace_hours, 48);
    assert_eq!(partial.max_upload_bytes, config.max_upload_bytes);
}

#[test]
fn upload_limit_at_bounds_is_accepted() {
    let yaml = format!("files:\n  documents:\n    maxUploadBytes: {MIN_DOCUMENT_UPLOAD_BYTES}\n");
    assert!(error_fields(&yaml).is_empty());
    let yaml = format!("files:\n  documents:\n    maxUploadBytes: {MAX_DOCUMENT_UPLOAD_BYTES}\n");
    assert!(error_fields(&yaml).is_empty());
}

#[test]
fn upload_limit_below_minimum_is_rejected() {
    let yaml = format!(
        "files:\n  documents:\n    maxUploadBytes: {}\n",
        MIN_DOCUMENT_UPLOAD_BYTES - 1
    );
    assert_eq!(error_fields(&yaml), vec!["files.documents.maxUploadBytes"]);
}

#[test]
fn upload_limit_above_maximum_is_rejected() {
    let yaml = format!(
        "files:\n  documents:\n    maxUploadBytes: {}\n",
        MAX_DOCUMENT_UPLOAD_BYTES + 1
    );
    assert_eq!(error_fields(&yaml), vec!["files.documents.maxUploadBytes"]);
}

#[test]
fn zero_grace_and_ttl_are_rejected() {
    let fields =
        error_fields("files:\n  documents:\n    blobGcGraceHours: 0\n    presenceTtlSecs: 0\n");
    assert_eq!(
        fields,
        vec![
            "files.documents.blobGcGraceHours",
            "files.documents.presenceTtlSecs"
        ]
    );
}

#[test]
fn the_pdf_page_cap_defaults_to_five_hundred_and_accepts_its_bounds() {
    assert_eq!(DocumentLimits::default().pdf_pages, 500);
    assert_eq!(DocumentsConfig::default().limits.pdf_pages, 500);
    let partial: DocumentsConfig =
        serde_yaml::from_str("limits:\n  pdfPages: 12\n").expect("parse");
    assert_eq!(partial.limits.pdf_pages, 12);
    assert_eq!(
        partial.limits.max_ops_per_call,
        DocumentLimits::default().max_ops_per_call
    );
    for pages in [1, 5000] {
        let yaml = format!("files:\n  documents:\n    limits:\n      pdfPages: {pages}\n");
        assert!(error_fields(&yaml).is_empty(), "pdfPages {pages}");
    }
}

#[test]
fn a_pdf_page_cap_outside_one_to_five_thousand_is_rejected() {
    for pages in [0, 5001] {
        let yaml = format!("files:\n  documents:\n    limits:\n      pdfPages: {pages}\n");
        assert_eq!(
            error_fields(&yaml),
            vec!["files.documents.limits.pdfPages"],
            "pdfPages {pages}"
        );
    }
}

#[test]
fn every_limit_is_rejected_just_outside_its_bounds_and_named_by_its_field() {
    for (field, _, (low, high)) in DocumentLimits::default().bounded() {
        for value in [low - 1, high + 1] {
            let yaml = format!("files:\n  documents:\n    limits:\n      {field}: {value}\n");
            assert_eq!(
                error_fields(&yaml),
                vec![format!("files.documents.limits.{field}")],
                "{field} {value}"
            );
        }
        let yaml = format!("files:\n  documents:\n    limits:\n      {field}: {high}\n");
        assert!(error_fields(&yaml).is_empty(), "{field} at its ceiling");
    }
}
