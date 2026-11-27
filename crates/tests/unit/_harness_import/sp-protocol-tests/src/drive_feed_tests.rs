use sp_protocol::ids::{DocumentId, DriveDeviceId};
use sp_protocol::wire::drive::{
    ChangeKind, CommitMeta, CommitOutcome, FeedChanges, FeedManifest, MoveRequest, SyncTokenAnswer,
    TrashOutcome, TrashRequest,
};
use sp_protocol::wire::paths;

const DOC: &str = "6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f";
const REV: &str = "11111111-2222-4333-8444-555555555555";

#[test]
fn the_manifest_and_change_feed_decode() {
    let manifest: FeedManifest = serde_json::from_value(serde_json::json!({
        "cursor": "c0",
        "entries": [{ "document_id": DOC, "path": "a.md", "head_revision_id": REV, "content_sha256": "ab" }]
    }))
    .unwrap();
    assert_eq!(manifest.entries[0].size_bytes, None);
    assert_eq!(manifest.entries[0].head_revision_id.as_str(), REV);
    let changes: FeedChanges = serde_json::from_value(serde_json::json!({
        "changes": [
            { "cursor": "c1", "document_id": DOC, "path": "a.md", "revision_id": REV, "content_sha256": "ab", "kind": { "type": "created" } },
            { "cursor": "c2", "document_id": DOC, "path": "b.md", "kind": { "type": "renamed", "from": "a.md" } },
            { "cursor": "c3", "document_id": DOC, "path": "b.md", "kind": { "type": "deleted" } }
        ],
        "next_cursor": "c3"
    }))
    .unwrap();
    assert_eq!(changes.changes[0].kind, ChangeKind::Created);
    assert_eq!(
        changes.changes[1].kind,
        ChangeKind::Renamed {
            from: Some("a.md".to_owned())
        }
    );
    assert!(changes.changes[1].revision_id.is_none());
    assert_eq!(changes.next_cursor, "c3");
}

#[test]
fn commit_and_trash_outcomes_decode() {
    let conflict: CommitOutcome = serde_json::from_value(serde_json::json!({
        "status": "conflict", "head_revision_id": REV, "head_content_sha256": "cd", "base_revision_id": null
    }))
    .unwrap();
    assert!(matches!(
        conflict,
        CommitOutcome::Conflict {
            base_revision_id: None,
            ..
        }
    ));
    let committed: CommitOutcome =
        serde_json::from_value(serde_json::json!({ "status": "committed", "revision_id": REV }))
            .unwrap();
    assert!(matches!(committed, CommitOutcome::Committed { .. }));
    let trashed: TrashOutcome =
        serde_json::from_value(serde_json::json!({ "status": "trashed" })).unwrap();
    assert_eq!(trashed, TrashOutcome::Trashed);
}

#[test]
fn commit_meta_leaves_out_what_it_lacks() {
    let meta = CommitMeta {
        document_id: None,
        path: "new.md".to_owned(),
        base_revision: None,
        content_sha256: "ab".to_owned(),
        size_bytes: 3,
        device_id: DriveDeviceId::try_new("install-1").unwrap(),
        message: None,
    };
    assert_eq!(
        serde_json::to_string(&meta).unwrap(),
        r#"{"path":"new.md","content_sha256":"ab","size_bytes":3,"device_id":"install-1"}"#
    );
}

#[test]
fn trash_and_move_bodies_name_the_document() {
    let id: DocumentId = serde_json::from_value(serde_json::json!(DOC)).unwrap();
    let trash = serde_json::to_value(TrashRequest { document_id: &id }).unwrap();
    assert_eq!(trash, serde_json::json!({ "document_id": DOC }));
    let moved = serde_json::to_value(MoveRequest {
        document_id: &id,
        path: "b/c.md",
    })
    .unwrap();
    assert_eq!(
        moved,
        serde_json::json!({ "document_id": DOC, "path": "b/c.md" })
    );
}

#[test]
fn the_sync_token_redacts_and_refuses_empty() {
    let answer: SyncTokenAnswer =
        serde_json::from_str(r#"{"token":"a.b.c","expires_at":"2026-11-01T09:00:00Z"}"#).unwrap();
    assert_eq!(answer.token.expose(), "a.b.c");
    assert!(!format!("{answer:?}").contains("a.b.c"));
    assert!(
        serde_json::from_str::<SyncTokenAnswer>(
            r#"{"token":"","expires_at":"2026-11-01T09:00:00Z"}"#
        )
        .is_err()
    );
}

#[test]
fn feed_routes_live_under_the_bridge_drive_prefix() {
    for route in [
        paths::DRIVE_MANIFEST,
        paths::DRIVE_CHANGES,
        paths::DRIVE_BLOB,
        paths::DRIVE_COMMIT,
        paths::DRIVE_TRASH,
        paths::DRIVE_MOVE,
        paths::DRIVE_TOKEN,
    ] {
        assert!(route.starts_with("/v1/bridge/drive/"), "{route}");
    }
    assert_eq!(paths::DRIVE_TOKEN, "/v1/bridge/drive/token");
}
