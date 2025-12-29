//! Unit tests for job-related identifier types.

use std::collections::HashSet;
use systemprompt_identifiers::{ScheduledJobId, JobName, ToDbValue, DbValue};

// ============================================================================
// ScheduledJobId Tests
// ============================================================================

#[test]
fn test_scheduled_job_id_new() {
    let id = ScheduledJobId::new("job-123");
    assert_eq!(id.as_str(), "job-123");
}

#[test]
fn test_scheduled_job_id_generate() {
    let id = ScheduledJobId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_scheduled_job_id_generate_unique() {
    let id1 = ScheduledJobId::generate();
    let id2 = ScheduledJobId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_scheduled_job_id_display() {
    let id = ScheduledJobId::new("display-job");
    assert_eq!(format!("{}", id), "display-job");
}

#[test]
fn test_scheduled_job_id_from_string() {
    let id: ScheduledJobId = String::from("from-string-job").into();
    assert_eq!(id.as_str(), "from-string-job");
}

#[test]
fn test_scheduled_job_id_from_str() {
    let id: ScheduledJobId = "from-str-job".into();
    assert_eq!(id.as_str(), "from-str-job");
}

#[test]
fn test_scheduled_job_id_as_ref() {
    let id = ScheduledJobId::new("as-ref-job");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-job");
}

#[test]
fn test_scheduled_job_id_clone_and_eq() {
    let id1 = ScheduledJobId::new("clone-job");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_scheduled_job_id_hash() {
    let id1 = ScheduledJobId::new("hash-job");
    let id2 = ScheduledJobId::new("hash-job");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_scheduled_job_id_serialize_json() {
    let id = ScheduledJobId::new("serialize-job");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-job\"");
}

#[test]
fn test_scheduled_job_id_deserialize_json() {
    let id: ScheduledJobId = serde_json::from_str("\"deserialize-job\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-job");
}

#[test]
fn test_scheduled_job_id_to_db_value() {
    let id = ScheduledJobId::new("db-value-job");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-job"));
}

#[test]
fn test_scheduled_job_id_ref_to_db_value() {
    let id = ScheduledJobId::new("db-value-ref-job");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-job"));
}

// ============================================================================
// JobName Tests
// ============================================================================

#[test]
fn test_job_name_new() {
    let name = JobName::new("my-job");
    assert_eq!(name.as_str(), "my-job");
}

#[test]
fn test_job_name_display() {
    let name = JobName::new("display-job");
    assert_eq!(format!("{}", name), "display-job");
}

#[test]
fn test_job_name_from_string() {
    let name: JobName = String::from("from-string-job").into();
    assert_eq!(name.as_str(), "from-string-job");
}

#[test]
fn test_job_name_from_str() {
    let name: JobName = "from-str-job".into();
    assert_eq!(name.as_str(), "from-str-job");
}

#[test]
fn test_job_name_as_ref() {
    let name = JobName::new("as-ref-job");
    let s: &str = name.as_ref();
    assert_eq!(s, "as-ref-job");
}

#[test]
fn test_job_name_clone_and_eq() {
    let name1 = JobName::new("clone-job");
    let name2 = name1.clone();
    assert_eq!(name1, name2);
}

#[test]
fn test_job_name_hash() {
    let name1 = JobName::new("hash-job");
    let name2 = JobName::new("hash-job");

    let mut set = HashSet::new();
    set.insert(name1.clone());
    assert!(set.contains(&name2));
}

#[test]
fn test_job_name_serialize_json() {
    let name = JobName::new("serialize-job");
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, "\"serialize-job\"");
}

#[test]
fn test_job_name_deserialize_json() {
    let name: JobName = serde_json::from_str("\"deserialize-job\"").unwrap();
    assert_eq!(name.as_str(), "deserialize-job");
}

#[test]
fn test_job_name_to_db_value() {
    let name = JobName::new("db-value-job");
    let db_value = name.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-job"));
}

#[test]
fn test_job_name_ref_to_db_value() {
    let name = JobName::new("db-value-ref-job");
    let db_value = (&name).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-job"));
}

#[test]
fn test_job_name_debug() {
    let name = JobName::new("debug-job");
    let debug_str = format!("{:?}", name);
    assert!(debug_str.contains("JobName"));
    assert!(debug_str.contains("debug-job"));
}

#[test]
fn test_job_name_empty_allowed() {
    let name = JobName::new("");
    assert_eq!(name.as_str(), "");
}

#[test]
fn test_job_name_descriptive() {
    let name = JobName::new("daily-cleanup-expired-sessions");
    assert_eq!(name.as_str(), "daily-cleanup-expired-sessions");
}
