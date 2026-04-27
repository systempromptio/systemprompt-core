//! Tests for ContentKind, IngestionReport, IngestionOptions, and
//! IngestionSource.

use systemprompt_content::{IngestionOptions, IngestionReport, IngestionSource};
use systemprompt_identifiers::{CategoryId, SourceId};

// ============================================================================
// ContentKind Tests
// ============================================================================

#[test]
fn test_content_kind_as_str_article() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Article;
    assert_eq!(kind.as_str(), "article");
}

#[test]
fn test_content_kind_as_str_guide() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Guide;
    assert_eq!(kind.as_str(), "guide");
}

#[test]
fn test_content_kind_as_str_tutorial() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Tutorial;
    assert_eq!(kind.as_str(), "tutorial");
}

#[test]
fn test_content_kind_display() {
    use systemprompt_content::models::ContentKind;
    assert_eq!(format!("{}", ContentKind::Article), "article");
    assert_eq!(format!("{}", ContentKind::Guide), "guide");
    assert_eq!(format!("{}", ContentKind::Tutorial), "tutorial");
}

#[test]
fn test_content_kind_default() {
    use systemprompt_content::models::ContentKind;
    let default_kind = ContentKind::default();
    assert_eq!(default_kind, ContentKind::Article);
}

#[test]
fn test_content_kind_serialization() {
    use systemprompt_content::models::ContentKind;
    let kind = ContentKind::Guide;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"guide\"");
}

// ============================================================================
// IngestionReport Tests
// ============================================================================

#[test]
fn test_ingestion_report_new() {
    let report = IngestionReport::new();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn test_ingestion_report_default() {
    let report = IngestionReport::default();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn test_ingestion_report_is_success_empty_errors() {
    let report = IngestionReport::new();
    assert!(report.is_success());
}

#[test]
fn test_ingestion_report_is_success_with_errors() {
    let mut report = IngestionReport::new();
    report.errors.push("Some error".to_string());
    assert!(!report.is_success());
}

#[test]
fn test_ingestion_report_with_data() {
    let mut report = IngestionReport::new();
    report.files_found = 10;
    report.files_processed = 8;
    report.errors.push("File not found".to_string());
    report.errors.push("Parse error".to_string());

    assert_eq!(report.files_found, 10);
    assert_eq!(report.files_processed, 8);
    assert_eq!(report.errors.len(), 2);
    assert!(!report.is_success());
}

// ============================================================================
// IngestionOptions Tests
// ============================================================================

#[test]
fn test_ingestion_options_default() {
    let options = IngestionOptions::default();
    assert!(!options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn test_ingestion_options_with_override() {
    let options = IngestionOptions::default().with_override(true);
    assert!(options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn test_ingestion_options_with_recursive() {
    let options = IngestionOptions::default().with_recursive(true);
    assert!(!options.override_existing);
    assert!(options.recursive);
}

#[test]
fn test_ingestion_options_builder_chain() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_recursive(true);
    assert!(options.override_existing);
    assert!(options.recursive);
}

#[test]
fn test_ingestion_options_with_override_false() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_override(false);
    assert!(!options.override_existing);
}

// ============================================================================
// IngestionSource Tests
// ============================================================================

#[test]
fn test_ingestion_source_new() {
    let source_id = SourceId::new("blog");
    let category_id = CategoryId::new("tech");
    let source = IngestionSource::new(&source_id, "blog", &category_id);

    assert_eq!(source.source_id.as_str(), "blog");
    assert_eq!(source.source_name, "blog");
    assert_eq!(source.category_id.as_str(), "tech");
}

#[test]
fn test_ingestion_source_different_ids() {
    let source_id = SourceId::new("docs");
    let category_id = CategoryId::new("documentation");
    let source = IngestionSource::new(&source_id, "docs", &category_id);

    assert_eq!(source.source_id.as_str(), "docs");
    assert_eq!(source.source_name, "docs");
    assert_eq!(source.category_id.as_str(), "documentation");
}

#[test]
fn test_ingestion_source_clone() {
    let source_id = SourceId::new("tutorials");
    let category_id = CategoryId::new("learning");
    let source = IngestionSource::new(&source_id, "tutorials", &category_id);
    let cloned = source.clone();

    assert_eq!(cloned.source_id, source.source_id);
    assert_eq!(cloned.source_name, source.source_name);
    assert_eq!(cloned.category_id, source.category_id);
}
