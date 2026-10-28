use std::path::PathBuf;

use systemprompt_bridge::drive::{DriveStatus, EditorPresence};
use systemprompt_bridge::verdict::Tone;
use systemprompt_bridge::wire::drive::{DriveCode, DriveStatusPayload};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::documents::{PresenceSource, PresenceState};

use crate::status::conflict;

fn synced() -> DriveStatus {
    DriveStatus {
        enabled: true,
        root: Some(PathBuf::from("/home/u/Drive")),
        synced_at: Some(chrono::Utc::now()),
        ..DriveStatus::default()
    }
}

#[test]
fn the_code_follows_the_status_precedence() {
    assert_eq!(DriveCode::of(&DriveStatus::default()), DriveCode::Off);
    assert_eq!(DriveCode::of(&synced()), DriveCode::Synced);

    let mut pending = synced();
    pending.pending_uploads = 2;
    assert_eq!(DriveCode::of(&pending), DriveCode::Pending);
    let mut never = synced();
    never.synced_at = None;
    assert_eq!(DriveCode::of(&never), DriveCode::Pending);

    let mut conflicted = pending.clone();
    conflicted.conflicts.push(conflict("a.md"));
    assert_eq!(DriveCode::of(&conflicted), DriveCode::Conflicts);

    let mut failed = conflicted.clone();
    failed.last_error = Some("boom".to_owned());
    assert_eq!(DriveCode::of(&failed), DriveCode::Error);
}

#[test]
fn each_code_carries_its_tone() {
    let tones = [
        (DriveCode::Off, Tone::Unknown),
        (DriveCode::Synced, Tone::Ok),
        (DriveCode::Pending, Tone::Probing),
        (DriveCode::Conflicts, Tone::Warn),
        (DriveCode::Error, Tone::Err),
    ];
    for (code, tone) in tones {
        let verdict = code.verdict();
        assert_eq!(verdict.tone, tone, "{code:?}");
        assert_eq!(verdict.code, code);
    }
}

#[test]
fn the_payload_projects_conflicts_and_editors() {
    let mut status = synced();
    let notice = conflict("notes/a.md");
    status.conflicts.push(notice.clone());
    let doc = DocumentId::generate();
    let seen = chrono::Utc::now();
    status.editors.push(EditorPresence {
        document_id: doc.clone(),
        path: Some("notes/a.md".to_owned()),
        user_id: UserId::new("alice"),
        source_kind: PresenceSource::Web,
        state: PresenceState::Editing,
        seen_at: seen,
    });
    status.editors.push(EditorPresence {
        document_id: doc.clone(),
        path: None,
        user_id: UserId::new("bob"),
        source_kind: PresenceSource::Agent,
        state: PresenceState::Viewing,
        seen_at: seen,
    });
    status.pending_uploads = 4;

    let payload = DriveStatusPayload::from(&status);

    assert_eq!(payload.verdict.code, DriveCode::Conflicts);
    assert_eq!(payload.root.as_deref(), Some("/home/u/Drive"));
    assert_eq!(
        payload.synced_at_unix,
        status.synced_at.map(|t| t.timestamp())
    );
    assert_eq!(payload.pending_uploads, 4);
    assert_eq!(payload.editing_count, 1);
    assert_eq!(payload.conflicts.len(), 1);
    assert_eq!(payload.conflicts[0].path, "notes/a.md");
    assert_eq!(payload.conflicts[0].sibling, notice.sibling);
    assert_eq!(
        payload.conflicts[0].head_revision_id,
        notice.head_revision_id.as_str()
    );
    assert_eq!(
        payload.conflicts[0].detected_at_unix,
        notice.detected_at.timestamp()
    );
    assert_eq!(payload.editors.len(), 2);
    assert!(payload.editors[0].editing);
    assert!(!payload.editors[1].editing);
    assert_eq!(payload.editors[0].path.as_deref(), Some("notes/a.md"));
    assert_eq!(payload.editors[1].seen_at_unix, seen.timestamp());

    let json = serde_json::to_value(&payload).expect("serialise");
    assert_eq!(json["verdict"]["code"], "conflicts");
}
