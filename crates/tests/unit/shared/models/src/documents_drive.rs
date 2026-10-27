use chrono::Utc;
use serde::Serialize;
use serde::de::DeserializeOwned;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, DriveDeviceId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::{
    DocumentEventName, DocumentFormat, DocumentRevisionCreatedEvent, DriveChange, DriveChangeKind,
    DriveChanges, DriveCommitMeta, DriveCommitResponse, DriveEntry, DriveManifest, PresenceBeacon,
    PresenceSource, PresenceState, content_hash,
};

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
    let json = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(&back, value);
}

#[test]
fn drive_types_round_trip() {
    let document_id = DocumentId::generate();
    let revision = DocumentRevisionId::generate();
    let sha = content_hash(b"bytes");
    round_trip(&DriveManifest {
        generated_at: Utc::now(),
        cursor: "c1".into(),
        entries: vec![DriveEntry {
            document_id: document_id.clone(),
            path: "Team/plan.docx".into(),
            format: DocumentFormat::Docx,
            head_revision_id: revision.clone(),
            content_sha256: sha.clone(),
            size_bytes: 5,
            updated_at: Utc::now(),
        }],
    });
    round_trip(&DriveChanges {
        changes: vec![DriveChange {
            cursor: "c2".into(),
            document_id: document_id.clone(),
            path: "Team/plan2.docx".into(),
            revision_id: Some(revision.clone()),
            content_sha256: Some(sha.clone()),
            kind: DriveChangeKind::Renamed {
                from: "Team/plan.docx".into(),
            },
        }],
        next_cursor: "c3".into(),
    });
    round_trip(&DriveCommitMeta {
        document_id: None,
        path: "new.md".into(),
        base_revision: None,
        content_sha256: sha,
        size_bytes: 5,
        device_id: DriveDeviceId::new("laptop"),
        message: None,
    });
    round_trip(&DriveCommitResponse::Conflict {
        head_revision_id: revision.clone(),
        base_revision_id: None,
    });
    round_trip(&PresenceBeacon {
        document_id,
        state: PresenceState::Editing,
        source_kind: PresenceSource::Bridge,
        source_ref: "laptop".into(),
        cursor: Some(serde_json::json!({ "line": 3 })),
    });
    let committed = serde_json::to_value(DriveCommitResponse::Committed {
        revision_id: revision,
    })
    .expect("ser");
    assert_eq!(committed["status"], "committed");
}

#[test]
fn event_names_match_payload_constants() {
    assert_eq!(
        DocumentRevisionCreatedEvent::NAME,
        "document.revision.created"
    );
    for name in DocumentEventName::ALL {
        assert_eq!(DocumentEventName::from_name(name.as_str()), Some(name));
        assert_eq!(serde_json::to_value(name).expect("ser"), name.as_str());
    }
    assert_eq!(
        DocumentEventName::CollabUpdate.as_str(),
        "document.collab.update"
    );
}

#[test]
fn api_paths_documents() {
    assert_eq!(ApiPaths::DOCUMENTS, "/api/v1/documents");
    assert_eq!(ApiPaths::DOCUMENT_FOLDERS, "/api/v1/document-folders");
    assert_eq!(ApiPaths::BRIDGE_DRIVE_MANIFEST, "/v1/bridge/drive/manifest");
    assert_eq!(ApiPaths::BRIDGE_DRIVE_CHANGES, "/v1/bridge/drive/changes");
    assert_eq!(ApiPaths::BRIDGE_DRIVE_COMMIT, "/v1/bridge/drive/commit");
    assert_eq!(ApiPaths::BRIDGE_DRIVE_PRESENCE, "/v1/bridge/drive/presence");
    assert_eq!(
        ApiPaths::BRIDGE_DRIVE_BLOB_ROUTE,
        "/v1/bridge/drive/blob/{sha256}"
    );
    let sha = content_hash(b"test");
    assert_eq!(
        ApiPaths::bridge_drive_blob(&sha),
        ApiPaths::BRIDGE_DRIVE_BLOB_ROUTE.replace("{sha256}", sha.as_str())
    );
}
