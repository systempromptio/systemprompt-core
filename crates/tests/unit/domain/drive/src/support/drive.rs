use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use systemprompt_models::ApiPaths;

use super::{FakeInstance, FakeRequest, FakeResponse, REV, REV2, sha256};

#[derive(Debug, Clone)]
pub(crate) struct Doc {
    pub(crate) id: &'static str,
    pub(crate) path: String,
    pub(crate) revision: &'static str,
    pub(crate) content: Vec<u8>,
}

impl Doc {
    pub(crate) fn new(id: &'static str, path: &str, content: &[u8]) -> Self {
        Self {
            id,
            path: path.to_owned(),
            revision: REV,
            content: content.to_vec(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct DriveState {
    pub(crate) documents: Vec<Doc>,
    pub(crate) tampered: BTreeMap<String, Vec<u8>>,
    pub(crate) changes: Vec<Value>,
    pub(crate) reset_cursor: bool,
    pub(crate) commit: Option<(u16, Value)>,
}

impl DriveState {
    fn entries(&self) -> Vec<Value> {
        self.documents
            .iter()
            .map(|doc| {
                json!({
                    "document_id": doc.id,
                    "path": doc.path,
                    "head_revision_id": doc.revision,
                    "content_sha256": sha256(&doc.content),
                })
            })
            .collect()
    }

    fn blob(&self, sha: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.tampered.get(sha) {
            return Some(bytes.clone());
        }
        self.documents
            .iter()
            .find(|doc| sha256(&doc.content) == sha)
            .map(|doc| doc.content.clone())
    }
}

pub(crate) type SharedDrive = Arc<Mutex<DriveState>>;

pub(crate) fn drive_state(documents: Vec<Doc>) -> SharedDrive {
    Arc::new(Mutex::new(DriveState {
        documents,
        ..DriveState::default()
    }))
}

pub(crate) async fn fake_drive(state: &SharedDrive) -> FakeInstance {
    let state = Arc::clone(state);
    FakeInstance::start(move |request| answer_feed(&mut state.lock().unwrap(), request)).await
}

fn answer_feed(state: &mut DriveState, request: &FakeRequest) -> FakeResponse {
    let path = request.path();
    let blob_prefix = format!("{}/", ApiPaths::BRIDGE_DRIVE_BLOB);
    if path == ApiPaths::BRIDGE_DRIVE_TOKEN {
        return FakeResponse::json(
            200,
            &json!({ "token": "sync.token.value", "expires_at": "2099-01-01T00:00:00Z" }),
        );
    }
    if path == ApiPaths::BRIDGE_DRIVE_MANIFEST {
        return FakeResponse::json(200, &json!({ "cursor": "c1", "entries": state.entries() }));
    }
    if path == ApiPaths::BRIDGE_DRIVE_CHANGES {
        if std::mem::take(&mut state.reset_cursor) {
            return FakeResponse::json(410, &json!({ "error_key": "cursor_reset" }));
        }
        let changes = std::mem::take(&mut state.changes);
        return FakeResponse::json(200, &json!({ "changes": changes, "next_cursor": "c2" }));
    }
    if path == ApiPaths::BRIDGE_DRIVE_COMMIT {
        let (status, outcome) = state
            .commit
            .clone()
            .unwrap_or_else(|| (200, json!({ "status": "committed", "revision_id": REV2 })));
        return FakeResponse::json(status, &outcome);
    }
    if path == ApiPaths::BRIDGE_DRIVE_MOVE {
        return FakeResponse::json(200, &json!({}));
    }
    if path == ApiPaths::BRIDGE_DRIVE_TRASH {
        return FakeResponse::json(200, &json!({ "status": "trashed" }));
    }
    path.strip_prefix(&blob_prefix)
        .and_then(|sha| state.blob(sha))
        .map_or_else(
            || FakeResponse::status(404),
            |bytes| FakeResponse::bytes(200, bytes),
        )
}

pub(crate) fn commits_naming(fake: &FakeInstance, rel: &str) -> Vec<FakeRequest> {
    let needle = format!("\"path\":\"{rel}\"");
    fake.to("POST", ApiPaths::BRIDGE_DRIVE_COMMIT)
        .into_iter()
        .filter(|request| request.text().contains(&needle))
        .collect()
}
