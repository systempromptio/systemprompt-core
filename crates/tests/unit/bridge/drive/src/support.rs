use std::path::Path;

use systemprompt_bridge::drive::{Cycle, DriveClient, DriveEntryState, DriveRoot, DriveState};
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::{
    ContentHash, DocumentId, DocumentRevisionId, DriveDeviceId, ValidatedUrl,
};
use systemprompt_models::documents::content_hash;

pub const HOST: &str = "laptop";

pub struct Fixture {
    pub _dir: tempfile::TempDir,
    pub root: DriveRoot,
    pub client: DriveClient,
    pub bearer: BearerToken,
    pub device: DriveDeviceId,
}

impl Fixture {
    pub fn new(server_uri: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let drive = dir.path().join("Drive");
        std::fs::create_dir_all(&drive).expect("drive dir");
        Self {
            root: DriveRoot::new(drive),
            client: DriveClient::new(
                ValidatedUrl::try_new(server_uri).expect("url"),
                reqwest::Client::new(),
            ),
            bearer: BearerToken::new("drive-bearer"),
            device: DriveDeviceId::try_new("device-1").expect("device id"),
            _dir: dir,
        }
    }

    pub fn cycle(&self) -> Cycle<'_> {
        Cycle {
            client: &self.client,
            bearer: &self.bearer,
            root: &self.root,
            device_id: &self.device,
            host_name: HOST,
        }
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.root.dir().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(path, bytes).expect("write");
    }

    pub fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.root.dir().join(rel)).expect("read")
    }

    pub fn state(&self) -> DriveState {
        DriveState::load(&self.root.state_path()).expect("state")
    }

    pub fn synced_empty(&self) {
        let mut state = self.state();
        state.cursor = Some("c0".to_owned());
        state.save(&self.root.state_path()).expect("save");
    }

    pub fn synced(&self, rel: &str, document_id: &DocumentId, revision: &DocumentRevisionId) {
        let path = self.root.dir().join(rel);
        let bytes = std::fs::read(&path).expect("read synced");
        let mut state = self.state();
        state.cursor = Some("c0".to_owned());
        state.entries.insert(
            rel.to_owned(),
            entry_for(&path, document_id, revision, content_hash(&bytes)),
        );
        state.save(&self.root.state_path()).expect("save");
    }
}

pub fn entry_for(
    path: &Path,
    document_id: &DocumentId,
    revision: &DocumentRevisionId,
    sha256: ContentHash,
) -> DriveEntryState {
    let (mtime_unix, size) = systemprompt_bridge::drive::scan::metadata(path).expect("metadata");
    DriveEntryState {
        document_id: Some(document_id.clone()),
        revision_id: revision.clone(),
        sha256,
        mtime_unix,
        size,
    }
}

pub fn manifest_json(
    entries: &[(&DocumentId, &str, &DocumentRevisionId, &[u8])],
) -> serde_json::Value {
    serde_json::json!({
        "generated_at": "2026-10-27T00:00:00Z",
        "cursor": "c-manifest",
        "entries": entries.iter().map(|(id, path, rev, bytes)| serde_json::json!({
            "document_id": id.as_str(),
            "path": path,
            "format": "markdown",
            "head_revision_id": rev.as_str(),
            "content_sha256": content_hash(bytes).as_str(),
            "size_bytes": bytes.len(),
            "updated_at": "2026-10-27T00:00:00Z",
        })).collect::<Vec<_>>(),
    })
}
