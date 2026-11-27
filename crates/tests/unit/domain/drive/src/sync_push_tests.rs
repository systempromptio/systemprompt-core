use serde_json::json;
use systemprompt_drive::sync::{ActivityKind, HeldChoice, HeldDeletes, HeldReason, SyncIndex};
use systemprompt_models::ApiPaths;

use crate::support::{
    DEVICE, DOC, DOC2, Doc, REV, commits_naming, document_id, drive_state, eventually, fake_drive,
    sha256, start_sync, status_where,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_edits_are_committed_and_ignored_files_stay_local() {
    let state = drive_state(vec![
        Doc::new(DOC, "a.md", b"one"),
        Doc::new(DOC2, "b.md", b"untouched"),
    ]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    status_where(&sync, "the first pull finished", |status| {
        status.counts.synced == 2
    })
    .await;
    std::fs::write(dir.path().join("a.md"), b"the second edit").unwrap();
    std::fs::write(dir.path().join("new.md"), b"fresh").unwrap();
    std::fs::write(dir.path().join("~$lock.docx"), b"lock").unwrap();
    std::fs::write(dir.path().join("notes.tmp"), b"scratch").unwrap();
    sync.sync_now();
    eventually("both files were committed", || {
        !commits_naming(&fake, "a.md").is_empty() && !commits_naming(&fake, "new.md").is_empty()
    })
    .await;
    let edit = commits_naming(&fake, "a.md").remove(0);
    let expected_match = format!("\"{REV}\"");
    assert_eq!(edit.header("if-match"), Some(expected_match.as_str()));
    let meta = edit.text();
    assert!(meta.contains(&format!("\"document_id\":\"{DOC}\"")));
    assert!(meta.contains(&format!("\"base_revision\":\"{REV}\"")));
    assert!(meta.contains(&format!(
        "\"content_sha256\":\"{}\"",
        sha256(b"the second edit")
    )));
    assert!(meta.contains(&format!("\"device_id\":\"{DEVICE}\"")));
    let created = commits_naming(&fake, "new.md").remove(0);
    assert_eq!(created.header("if-match"), None);
    assert!(!created.text().contains("base_revision"));
    for local_only in ["b.md", "~$lock.docx", "notes.tmp"] {
        assert!(
            commits_naming(&fake, local_only).is_empty(),
            "{local_only} was committed"
        );
    }
    let status = status_where(&sync, "the upload was reported", |status| {
        status
            .recent
            .iter()
            .any(|item| item.path == "a.md" && item.kind == ActivityKind::Uploaded)
    })
    .await;
    assert_eq!(status.counts.pending_uploads, 0);
    sync.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_local_rename_moves_the_document() {
    let state = drive_state(vec![Doc::new(DOC, "old.md", b"same words")]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    status_where(&sync, "the first pull finished", |status| {
        status.counts.synced == 1
    })
    .await;
    std::fs::rename(dir.path().join("old.md"), dir.path().join("new.md")).unwrap();
    sync.sync_now();
    eventually("the move reached the instance", || {
        !fake.to("POST", ApiPaths::BRIDGE_DRIVE_MOVE).is_empty()
    })
    .await;
    let moved = fake
        .to("POST", ApiPaths::BRIDGE_DRIVE_MOVE)
        .remove(0)
        .json();
    assert_eq!(moved, json!({ "document_id": DOC, "path": "new.md" }));
    let index_path = dir.path().join(".drive-state.json");
    eventually("the index follows the move", || {
        SyncIndex::load(&index_path)
            .is_ok_and(|index| index.path_of(&document_id(DOC)) == Some("new.md"))
    })
    .await;
    assert!(commits_naming(&fake, "new.md").is_empty());
    assert!(fake.to("POST", ApiPaths::BRIDGE_DRIVE_TRASH).is_empty());
    sync.stop().await;
}

const MANY: [&str; 6] = [
    "00000000-0000-4000-8000-000000000001",
    "00000000-0000-4000-8000-000000000002",
    "00000000-0000-4000-8000-000000000003",
    "00000000-0000-4000-8000-000000000004",
    "00000000-0000-4000-8000-000000000005",
    "00000000-0000-4000-8000-000000000006",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mass_deletes_are_held_and_can_be_restored() {
    let documents = MANY
        .into_iter()
        .enumerate()
        .map(|(n, id)| Doc::new(id, &format!("{n}.md"), format!("file {n}").as_bytes()))
        .collect();
    let state = drive_state(documents);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    status_where(&sync, "the first pull finished", |status| {
        status.counts.synced == 6
    })
    .await;
    for n in 0..4 {
        std::fs::remove_file(dir.path().join(format!("{n}.md"))).unwrap();
    }
    sync.sync_now();
    let status = status_where(&sync, "the deletes were held", |status| {
        status.held_deletes.is_some()
    })
    .await;
    assert_eq!(
        status.held_deletes,
        Some(HeldDeletes {
            reason: HeldReason::Bulk,
            count: 4
        })
    );
    assert_eq!(status.counts.held_deletes, 4);
    assert!(fake.to("POST", ApiPaths::BRIDGE_DRIVE_TRASH).is_empty());
    sync.choose_held_deletes(HeldChoice::Restore).await.unwrap();
    let restored = status_where(&sync, "the files came back", |status| {
        status.held_deletes.is_none()
    })
    .await;
    assert_eq!(restored.counts.synced, 6);
    for n in 0..4 {
        assert_eq!(
            std::fs::read(dir.path().join(format!("{n}.md"))).unwrap(),
            format!("file {n}").into_bytes()
        );
    }
    assert!(fake.to("POST", ApiPaths::BRIDGE_DRIVE_TRASH).is_empty());
    sync.stop().await;
}
