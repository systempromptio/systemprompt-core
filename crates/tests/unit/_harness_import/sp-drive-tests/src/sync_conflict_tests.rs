use serde_json::json;
use sp_drive::sync::ActivityKind;

use crate::support::{DOC, Doc, REV2, drive_state, fake_drive, start_sync, status_where};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_conflicting_edit_is_kept_beside_the_head() {
    let state = drive_state(vec![Doc::new(DOC, "a.md", b"base")]);
    let fake = fake_drive(&state).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    status_where(&sync, "the first pull finished", |status| {
        status.counts.synced == 1
    })
    .await;
    {
        let mut drive = state.lock().unwrap();
        drive.documents[0].revision = REV2;
        drive.documents[0].content = b"theirs".to_vec();
        drive.commit = Some((
            409,
            json!({ "status": "conflict", "head_revision_id": REV2 }),
        ));
    }
    std::fs::write(dir.path().join("a.md"), b"mine, edited").unwrap();
    sync.sync_now();
    let status = status_where(&sync, "the conflict was kept", |status| {
        !status.conflicts.is_empty()
    })
    .await;
    sync.stop().await;
    let notice = &status.conflicts[0];
    assert_eq!(notice.path, "a.md");
    assert_eq!(notice.sibling, "a (conflict from pc).md");
    assert!(!notice.unplaced);
    assert_eq!(notice.head_revision_id.to_string(), REV2);
    assert_eq!(std::fs::read(dir.path().join("a.md")).unwrap(), b"theirs");
    assert_eq!(
        std::fs::read(dir.path().join("a (conflict from pc).md")).unwrap(),
        b"mine, edited"
    );
    assert!(status.recent.iter().any(|item| {
        item.path == "a.md"
            && item.kind
                == ActivityKind::Conflict {
                    kept_as: "a (conflict from pc).md".to_owned(),
                }
    }));
}
