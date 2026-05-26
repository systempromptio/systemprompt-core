//! Tests for the bootstrap job structs: name/description/schedule/tags/enabled
//! accessors do not require a `JobContext`.

use systemprompt_sync::{AccessControlSyncJob, ContentSyncJob};
use systemprompt_traits::Job;

#[test]
fn access_control_sync_job_accessors() {
    let job = AccessControlSyncJob;
    assert_eq!(job.name(), "access_control_sync");
    assert!(!job.description().is_empty());
    assert_eq!(job.schedule(), "");
    let tags = job.tags();
    assert!(tags.contains(&"access-control"));
    assert!(tags.contains(&"sync"));
    assert!(tags.contains(&"bootstrap"));
    assert!(!job.enabled());
    let dbg = format!("{job:?}");
    assert!(dbg.contains("AccessControlSyncJob"));
}

#[test]
fn content_sync_job_name_and_description() {
    let job = ContentSyncJob;
    assert_eq!(job.name(), "content_sync");
    assert!(!job.description().is_empty());
    assert_eq!(job.schedule(), "");
    let dbg = format!("{job:?}");
    assert!(dbg.contains("ContentSyncJob"));
}

#[test]
fn access_control_sync_job_copy_clone() {
    let job = AccessControlSyncJob;
    let copy = job;
    let _ = copy.name();
}

#[test]
fn content_sync_job_copy_clone() {
    let job = ContentSyncJob;
    let copy = job;
    let _ = copy.name();
}
