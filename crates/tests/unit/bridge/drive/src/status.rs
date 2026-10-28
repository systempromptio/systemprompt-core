use std::path::PathBuf;

use systemprompt_bridge::drive::{ConflictNotice, DriveHandle, DriveRoot, DriveStatus};
use systemprompt_bridge::proxy::drive_signal::{DocumentNotice, NoticeKind};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, UserId};
use systemprompt_models::documents::{DocumentPresenceEvent, PresenceSource, PresenceState};

fn notice(n: usize) -> DocumentNotice {
    DocumentNotice {
        kind: NoticeKind::Shared,
        document_id: DocumentId::generate(),
        from: UserId::new(format!("user-{n}")),
        from_name: None,
        document_name: None,
        proposal_id: None,
        detail: None,
        message: None,
    }
}

fn presence(document_id: &DocumentId, user: &str, state: PresenceState) -> DocumentPresenceEvent {
    DocumentPresenceEvent {
        document_id: document_id.clone(),
        user_id: UserId::new(user),
        state,
        source_kind: PresenceSource::Web,
        source_ref: "tab-1".to_owned(),
        cursor: None,
    }
}

#[test]
fn a_status_read_without_a_folder_is_the_disabled_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = DriveRoot::new(dir.path().join("absent"));

    let status = DriveStatus::from_disk(&root).expect("status");

    assert_eq!(status, DriveStatus::default());
}

#[test]
fn a_status_read_from_disk_counts_unsynced_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let drive = dir.path().join("Drive");
    std::fs::create_dir_all(&drive).expect("mkdir");
    std::fs::write(drive.join("new.md"), b"fresh").expect("write");
    std::fs::write(drive.join(".hidden"), b"ignored").expect("write");
    let root = DriveRoot::new(drive.clone());

    let status = DriveStatus::from_disk(&root).expect("status");

    assert!(status.enabled);
    assert_eq!(status.root, Some(drive));
    assert_eq!(status.pending_uploads, 1);
    assert!(status.synced_at.is_none());
}

#[test]
fn notices_are_numbered_and_only_the_newest_twenty_are_kept() {
    let handle = DriveHandle::new();
    for n in 0..25 {
        handle.push_notice(notice(n));
    }

    let notices = handle.status().notices.clone();

    assert_eq!(notices.len(), 20);
    assert_eq!(notices.first().map(|q| q.seq), Some(6));
    assert_eq!(notices.last().map(|q| q.seq), Some(25));
    assert_eq!(
        notices.last().map(|q| q.notice.from.clone()),
        Some(UserId::new("user-24"))
    );
}

#[test]
fn a_repeated_presence_replaces_the_editor_and_counts_only_editing() {
    let handle = DriveHandle::new();
    let doc = DocumentId::generate();

    handle.observe_presence(&presence(&doc, "alice", PresenceState::Viewing), None);
    handle.observe_presence(
        &presence(&doc, "alice", PresenceState::Editing),
        Some("a.md".to_owned()),
    );
    handle.observe_presence(&presence(&doc, "bob", PresenceState::Viewing), None);

    let status = handle.status();
    assert_eq!(status.editors.len(), 2);
    assert_eq!(status.editing_count(), 1);
    let alice = status
        .editors
        .iter()
        .find(|e| e.user_id == UserId::new("alice"))
        .expect("alice");
    assert_eq!(alice.path.as_deref(), Some("a.md"));
    assert_eq!(alice.state, PresenceState::Editing);
}

#[test]
fn touched_paths_are_drained_once() {
    let handle = DriveHandle::new();
    handle.touch(PathBuf::from("/d/a.md"));
    handle.touch(PathBuf::from("/d/a.md"));
    handle.touch(PathBuf::from("/d/b.md"));

    assert_eq!(handle.take_touched().len(), 2);
    assert!(handle.take_touched().is_empty());
}

#[test]
fn an_unchanged_update_does_not_bump_the_generation() {
    let handle = DriveHandle::new();
    let rx = handle.subscribe();
    handle.update(|s| s.pending_uploads = 3);
    let after_change = *rx.borrow();
    handle.update(|s| s.pending_uploads = 3);

    assert_eq!(after_change, 1);
    assert_eq!(*rx.borrow(), 1);
}

#[tokio::test]
async fn sync_now_wakes_a_waiter() {
    let handle = DriveHandle::new();
    handle.sync_now();
    tokio::time::timeout(std::time::Duration::from_secs(2), handle.woken())
        .await
        .expect("woken");
}

pub fn conflict(path: &str) -> ConflictNotice {
    ConflictNotice {
        path: path.to_owned(),
        sibling: format!("{path} (conflict)"),
        head_revision_id: DocumentRevisionId::generate(),
        detected_at: chrono::Utc::now(),
    }
}
