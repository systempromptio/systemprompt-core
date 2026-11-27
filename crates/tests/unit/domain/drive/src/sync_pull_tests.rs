use serde_json::json;
use systemprompt_drive::sync::{ActivityKind, SyncIndex, SyncProblemKind};
use systemprompt_drive::{DriveStatus, SyncPhase};

use crate::support::{
    DOC, DOC2, DOC3, Doc, document_id, drive_state, eventually, fake_drive, sha256, start_sync,
    status_where,
};

const INDEX: &str = ".drive-state.json";

fn activity(status: &DriveStatus, path: &str, wanted: fn(&ActivityKind) -> bool) -> bool {
    status
        .recent
        .iter()
        .any(|item| item.path == path && wanted(&item.kind))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_first_pull_places_what_this_device_can_hold() {
    let state = drive_state(vec![
        Doc::new(DOC, "Reports/q3.md", b"third quarter"),
        Doc::new(DOC2, "Reports/Q3.md", b"shouting"),
        Doc::new(DOC3, "CON.md", b"reserved"),
    ]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let expired = dir.path().join(".trash").join("20200101T000000.000Z");
    std::fs::create_dir_all(&expired).unwrap();
    std::fs::write(expired.join("old.md"), b"old").unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    let status = status_where(&sync, "the first pull finished", |status| {
        status.last_synced.is_some()
    })
    .await;
    assert_eq!(
        std::fs::read(dir.path().join("Reports").join("q3.md")).unwrap(),
        b"third quarter"
    );
    assert_eq!(status.counts.synced, 1);
    assert_eq!(status.conflicts.len(), 2);
    assert!(status.conflicts.iter().all(|notice| notice.unplaced));
    let collided = status
        .conflicts
        .iter()
        .find(|notice| notice.path == "Reports/Q3.md")
        .unwrap();
    assert_eq!(collided.sibling, "Reports/q3.md");
    assert!(
        status
            .conflicts
            .iter()
            .any(|notice| notice.path == "CON.md")
    );
    assert!(activity(&status, "Reports/q3.md", |kind| matches!(
        kind,
        ActivityKind::Downloaded
    )));
    assert!(activity(&status, "CON.md", |kind| matches!(
        kind,
        ActivityKind::NotPlaced { .. }
    )));
    assert!(!expired.exists());
    let index = SyncIndex::load(&dir.path().join(INDEX)).unwrap();
    assert_eq!(index.path_of(&document_id(DOC)), Some("Reports/q3.md"));
    assert!(index.cursor.is_some());
    sync.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_path_outside_the_folder_fails_the_cycle() {
    let state = drive_state(vec![Doc::new(DOC, "../escape.md", b"out")]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, &dir.path().join("Drive")).await;
    let status = status_where(&sync, "the cycle failed", |status| {
        status.phase == SyncPhase::Error
    })
    .await;
    assert_eq!(
        status.problem.map(|problem| problem.kind),
        Some(SyncProblemKind::LocalFiles)
    );
    assert!(!dir.path().join("escape.md").exists());
    sync.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_blob_that_does_not_match_its_address_is_not_written() {
    let state = drive_state(vec![Doc::new(DOC, "a.md", b"expected")]);
    state
        .lock()
        .unwrap()
        .tampered
        .insert(sha256(b"expected"), b"tampered".to_vec());
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    let status = status_where(&sync, "the cycle failed", |status| {
        status.phase == SyncPhase::Error
    })
    .await;
    assert_eq!(
        status.problem.map(|problem| problem.kind),
        Some(SyncProblemKind::Failed)
    );
    assert!(!dir.path().join("a.md").exists());
    sync.stop().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_path_under_a_link_is_not_placed() {
    let state = drive_state(vec![Doc::new(DOC, "linked/x.md", b"x")]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = dir.path().join("Drive");
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("linked")).unwrap();
    let sync = start_sync(&fake, &root).await;
    let status = status_where(&sync, "the pull finished", |status| {
        status.last_synced.is_some()
    })
    .await;
    assert!(
        status
            .conflicts
            .iter()
            .any(|notice| notice.unplaced && notice.path == "linked/x.md")
    );
    assert!(!outside.path().join("x.md").exists());
    sync.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_remote_delete_moves_the_copy_to_the_trash() {
    let state = drive_state(vec![Doc::new(DOC, "a.md", b"short-lived")]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    status_where(&sync, "the first pull finished", |status| {
        status.counts.synced == 1
    })
    .await;
    {
        let mut drive = state.lock().unwrap();
        drive.documents.clear();
        drive.changes.push(json!({ "cursor": "c2", "document_id": DOC, "path": "a.md", "kind": { "type": "deleted" } }));
        drop(drive);
    }
    sync.sync_now();
    let copy = dir.path().join("a.md");
    eventually("the copy left the folder", || !copy.exists()).await;
    let trashed = std::fs::read_dir(dir.path().join(".trash"))
        .unwrap()
        .filter_map(Result::ok)
        .any(|stamp| stamp.path().join("a.md").exists());
    assert!(trashed);
    let status = status_where(&sync, "the delete was reported", |status| {
        activity(status, "a.md", |kind| {
            matches!(kind, ActivityKind::DeletedRemotely)
        })
    })
    .await;
    assert_eq!(status.counts.synced, 0);
    sync.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reset_cursor_reads_the_manifest_again() {
    let state = drive_state(vec![Doc::new(DOC, "a.md", b"kept")]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    status_where(&sync, "the first pull finished", |status| {
        status.counts.synced == 1
    })
    .await;
    let manifests = fake
        .to("GET", systemprompt_models::ApiPaths::BRIDGE_DRIVE_MANIFEST)
        .len();
    state.lock().unwrap().reset_cursor = true;
    sync.sync_now();
    eventually("the manifest was read again", || {
        let consumed = !state.lock().unwrap().reset_cursor;
        consumed
            && fake
                .to("GET", systemprompt_models::ApiPaths::BRIDGE_DRIVE_MANIFEST)
                .len()
                > manifests
    })
    .await;
    let status = status_where(&sync, "the cycle settled", |status| {
        status.phase == SyncPhase::Idle
    })
    .await;
    assert!(status.problem.is_none());
    assert_eq!(status.counts.synced, 1);
    assert_eq!(std::fs::read(dir.path().join("a.md")).unwrap(), b"kept");
    sync.stop().await;
}
