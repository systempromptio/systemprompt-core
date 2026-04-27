use systemprompt_content::models::ContentKind;
use systemprompt_content::{ContentMetadata, IngestionOptions, IngestionReport};

#[test]
fn content_kind_deserialize_article() {
    let json = "\"article\"";
    let kind: ContentKind = serde_json::from_str(json).unwrap();
    assert_eq!(kind, ContentKind::Article);
}

#[test]
fn content_kind_deserialize_guide() {
    let json = "\"guide\"";
    let kind: ContentKind = serde_json::from_str(json).unwrap();
    assert_eq!(kind, ContentKind::Guide);
}

#[test]
fn content_kind_deserialize_tutorial() {
    let json = "\"tutorial\"";
    let kind: ContentKind = serde_json::from_str(json).unwrap();
    assert_eq!(kind, ContentKind::Tutorial);
}

#[test]
fn content_kind_roundtrip_all_variants() {
    for kind in [
        ContentKind::Article,
        ContentKind::Guide,
        ContentKind::Tutorial,
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: ContentKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kind);
    }
}

#[test]
fn content_kind_equality() {
    assert_eq!(ContentKind::Article, ContentKind::Article);
    assert_ne!(ContentKind::Article, ContentKind::Guide);
    assert_ne!(ContentKind::Guide, ContentKind::Tutorial);
}

#[test]
fn content_kind_copy_semantics() {
    let kind = ContentKind::Guide;
    let copied = kind;
    assert_eq!(kind, copied);
}

#[test]
fn ingestion_options_with_dry_run_true() {
    let options = IngestionOptions::default().with_dry_run(true);
    assert!(options.dry_run);
    assert!(!options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn ingestion_options_with_dry_run_false() {
    let options = IngestionOptions::default()
        .with_dry_run(true)
        .with_dry_run(false);
    assert!(!options.dry_run);
}

#[test]
fn ingestion_options_full_chain_with_dry_run() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_recursive(true)
        .with_dry_run(true);
    assert!(options.override_existing);
    assert!(options.recursive);
    assert!(options.dry_run);
}

#[test]
fn ingestion_report_serde_roundtrip_empty() {
    let report = IngestionReport::new();
    let json = serde_json::to_string(&report).unwrap();
    let deserialized: IngestionReport = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.files_found, 0);
    assert_eq!(deserialized.files_processed, 0);
    assert!(deserialized.errors.is_empty());
    assert!(deserialized.warnings.is_empty());
    assert!(deserialized.is_success());
}

#[test]
fn ingestion_report_serde_roundtrip_with_data() {
    let mut report = IngestionReport::new();
    report.files_found = 50;
    report.files_processed = 45;
    report.errors.push("parse error in file.md".to_string());
    report.warnings.push("duplicate slug detected".to_string());
    report.unchanged_count = 10;
    report.skipped_count = 3;

    let json = serde_json::to_string(&report).unwrap();
    let deserialized: IngestionReport = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.files_found, 50);
    assert_eq!(deserialized.files_processed, 45);
    assert_eq!(deserialized.errors.len(), 1);
    assert_eq!(deserialized.warnings.len(), 1);
    assert_eq!(deserialized.unchanged_count, 10);
    assert_eq!(deserialized.skipped_count, 3);
    assert!(!deserialized.is_success());
}

#[test]
fn ingestion_report_would_create_and_would_update_skipped_when_empty() {
    let report = IngestionReport::new();
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("would_create"));
    assert!(!json.contains("would_update"));
}

#[test]
fn ingestion_report_would_create_present_when_nonempty() {
    let mut report = IngestionReport::new();
    report.would_create.push("new-article".to_string());
    report.would_update.push("existing-article".to_string());

    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("would_create"));
    assert!(json.contains("new-article"));
    assert!(json.contains("would_update"));
    assert!(json.contains("existing-article"));
}

#[test]
fn content_metadata_public_defaults_to_none_when_absent() {
    let yaml = r#"
title: Test
slug: test
published_at: "2024-01-01"
kind: article
"#;
    let metadata: ContentMetadata = serde_yaml::from_str(yaml).unwrap();
    assert!(metadata.public.is_none());
}

#[test]
fn content_metadata_public_deserializes_true() {
    let yaml = r#"
title: Test
slug: test
published_at: "2024-01-01"
kind: article
public: true
"#;
    let metadata: ContentMetadata = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(metadata.public, Some(true));
}

#[test]
fn content_metadata_public_deserializes_false() {
    let yaml = r#"
title: Test
slug: test
published_at: "2024-01-01"
kind: article
public: false
"#;
    let metadata: ContentMetadata = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(metadata.public, Some(false));
}
