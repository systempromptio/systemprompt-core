use chrono::Utc;
use serde_json::Value;
use sp_drive::sync::{HeldDeletes, HeldReason, IndexEntry, PendingDelete, SyncError, SyncIndex};

use crate::support::{DOC, REV, document_id, revision_id};

fn entry() -> IndexEntry {
    IndexEntry {
        document_id: Some(document_id(DOC)),
        revision_id: revision_id(REV),
        sha256: "ab".repeat(32),
        mtime_unix: 1_764_000_000,
        size: 42,
    }
}

#[test]
fn the_index_round_trips_through_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".drive-state.json");
    assert_eq!(SyncIndex::load(&path).unwrap(), SyncIndex::default());
    let mut index = SyncIndex {
        instance: Some("https://sp.example.com".to_owned()),
        cursor: Some("c-7".to_owned()),
        ..SyncIndex::default()
    };
    index.entries.insert("Reports/q3.md".to_owned(), entry());
    index.pending_deletes.insert(
        "old.md".to_owned(),
        PendingDelete {
            noticed_at: Utc::now(),
            cycles: 1,
        },
    );
    index.held_deletes = Some(HeldDeletes {
        reason: HeldReason::Bulk,
        count: 12,
    });
    index.save(&path).unwrap();
    let loaded = SyncIndex::load(&path).unwrap();
    assert_eq!(loaded, index);
    assert_eq!(loaded.path_of(&document_id(DOC)), Some("Reports/q3.md"));
    let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(raw.get("conflicts").is_none());
    assert_eq!(raw["held_deletes"]["reason"], "bulk");
}

#[test]
fn a_corrupt_index_is_moved_aside() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".drive-state.json");
    std::fs::write(&path, b"{not json").unwrap();
    assert!(matches!(
        SyncIndex::load(&path),
        Err(SyncError::IndexCorrupt { .. })
    ));
    assert!(!path.exists());
    let moved: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(moved.len(), 1);
    assert!(moved[0].starts_with(".drive-state.json.corrupt-"));
}

#[test]
fn notices_are_deduplicated_and_bounded() {
    let mut index = SyncIndex::default();
    index.record_notice("a.md", "refused".to_owned());
    index.record_notice("a.md", "refused".to_owned());
    assert_eq!(index.delete_notices.len(), 1);
    for n in 0..30 {
        index.record_notice(&format!("{n}.md"), "x".to_owned());
    }
    assert_eq!(index.delete_notices.len(), 20);
    assert_eq!(
        index
            .delete_notices
            .last()
            .map(|notice| notice.path.as_str()),
        Some("29.md")
    );
}
