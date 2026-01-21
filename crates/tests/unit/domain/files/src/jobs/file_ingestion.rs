//! Unit tests for FileIngestionJob

use systemprompt_files::FileIngestionJob;
use systemprompt_traits::Job;

// ============================================================================
// FileIngestionJob Construction Tests
// ============================================================================

#[test]
fn test_file_ingestion_job_new() {
    let job = FileIngestionJob::new();
    assert_eq!(job.name(), "file_ingestion");
}

#[test]
fn test_file_ingestion_job_default() {
    let job = FileIngestionJob::default();
    assert_eq!(job.name(), "file_ingestion");
}

#[test]
fn test_file_ingestion_job_new_equals_default() {
    let job1 = FileIngestionJob::new();
    let job2 = FileIngestionJob::default();

    assert_eq!(job1.name(), job2.name());
    assert_eq!(job1.description(), job2.description());
    assert_eq!(job1.schedule(), job2.schedule());
}

// ============================================================================
// Job Trait Implementation Tests
// ============================================================================

#[test]
fn test_file_ingestion_job_name() {
    let job = FileIngestionJob::new();
    assert_eq!(job.name(), "file_ingestion");
}

#[test]
fn test_file_ingestion_job_description() {
    let job = FileIngestionJob::new();
    let desc = job.description();

    assert!(!desc.is_empty());
    assert!(desc.contains("image") || desc.contains("file") || desc.contains("storage"));
}

#[test]
fn test_file_ingestion_job_schedule() {
    let job = FileIngestionJob::new();
    let schedule = job.schedule();

    assert!(!schedule.is_empty());
    assert!(schedule.contains('*'));
}

#[test]
fn test_file_ingestion_job_schedule_is_valid_cron() {
    let job = FileIngestionJob::new();
    let schedule = job.schedule();

    let parts: Vec<&str> = schedule.split_whitespace().collect();
    assert_eq!(parts.len(), 6, "Cron expression should have 6 parts");
}

#[test]
fn test_file_ingestion_job_enabled() {
    let job = FileIngestionJob::new();
    assert!(job.enabled());
}

// ============================================================================
// Copy/Clone Tests
// ============================================================================

#[test]
fn test_file_ingestion_job_copy() {
    let job = FileIngestionJob::new();
    let copied = job;

    assert_eq!(job.name(), copied.name());
}

#[test]
fn test_file_ingestion_job_clone() {
    let job = FileIngestionJob::new();
    let cloned = job.clone();

    assert_eq!(job.name(), cloned.name());
}

// ============================================================================
// Debug Implementation Tests
// ============================================================================

#[test]
fn test_file_ingestion_job_debug() {
    let job = FileIngestionJob::new();
    let debug_str = format!("{:?}", job);

    assert!(debug_str.contains("FileIngestionJob"));
}
