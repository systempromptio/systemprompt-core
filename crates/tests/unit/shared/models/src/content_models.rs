use systemprompt_models::content::{ContentLink, IngestionReport};

#[test]
fn content_link_new_sets_title_and_url() {
    let link = ContentLink::new("Rust Book", "https://doc.rust-lang.org/book/");
    assert_eq!(link.title, "Rust Book");
    assert_eq!(link.url, "https://doc.rust-lang.org/book/");
}

#[test]
fn content_link_serde_round_trip() {
    let link = ContentLink::new("Guide", "https://example.com/guide");
    let json = serde_json::to_string(&link).unwrap();
    let decoded: ContentLink = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.title, "Guide");
    assert_eq!(decoded.url, "https://example.com/guide");
}

#[test]
fn content_link_equality() {
    let a = ContentLink::new("t", "u");
    let b = ContentLink::new("t", "u");
    assert_eq!(a, b);
}

#[test]
fn ingestion_report_new_is_default() {
    let r = IngestionReport::new();
    assert_eq!(r.files_found, 0);
    assert_eq!(r.files_processed, 0);
    assert!(r.errors.is_empty());
    assert!(!r.has_errors());
}

#[test]
fn ingestion_report_has_errors_when_errors_present() {
    let r = IngestionReport {
        files_found: 5,
        files_processed: 3,
        errors: vec!["file1.md: parse error".to_owned()],
    };
    assert!(r.has_errors());
}

#[test]
fn ingestion_report_successful_count() {
    let r = IngestionReport {
        files_found: 10,
        files_processed: 8,
        errors: vec!["e1".to_owned(), "e2".to_owned()],
    };
    assert_eq!(r.successful_count(), 6);
    assert_eq!(r.failed_count(), 2);
}

#[test]
fn ingestion_report_successful_count_no_errors() {
    let r = IngestionReport {
        files_found: 5,
        files_processed: 5,
        errors: vec![],
    };
    assert_eq!(r.successful_count(), 5);
    assert_eq!(r.failed_count(), 0);
}

#[test]
fn ingestion_report_saturating_sub_when_errors_exceed_processed() {
    let r = IngestionReport {
        files_found: 3,
        files_processed: 1,
        errors: vec!["e1".to_owned(), "e2".to_owned(), "e3".to_owned()],
    };
    assert_eq!(r.successful_count(), 0);
}

#[test]
fn ingestion_report_default_matches_new() {
    let default = IngestionReport::default();
    let new = IngestionReport::new();
    assert_eq!(default.files_found, new.files_found);
    assert_eq!(default.files_processed, new.files_processed);
    assert_eq!(default.errors.len(), new.errors.len());
}
