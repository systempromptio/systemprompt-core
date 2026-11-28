//! The in-memory Drive: documents at their head revision, the blobs they
//! name, and the change feed every write appends to.

use std::collections::{BTreeMap, HashMap};

use chrono::Utc;
use systemprompt_identifiers::{ContentHash, DocumentId, DocumentRevisionId};
use systemprompt_models::documents::{DriveChange, DriveChangeKind, DriveEntry, content_hash};

use super::format_of;

#[derive(Debug, Default)]
pub(crate) struct DriveStore {
    pub(super) documents: BTreeMap<DocumentId, DriveEntry>,
    pub(super) blobs: HashMap<ContentHash, Vec<u8>>,
    pub(super) changes: Vec<DriveChange>,
}

impl DriveStore {
    pub(crate) fn holding(documents: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        let mut store = Self::default();
        for (path, bytes) in documents {
            store.write(None, path, bytes);
        }
        store
    }

    pub(super) fn write(
        &mut self,
        proposed: Option<DocumentId>,
        path: String,
        bytes: Vec<u8>,
    ) -> DocumentRevisionId {
        let held = self.held(proposed.as_ref(), &path);
        let kind = if held.is_some() {
            DriveChangeKind::Updated
        } else {
            DriveChangeKind::Created
        };
        let entry = DriveEntry {
            document_id: held
                .map(|head| head.document_id)
                .or(proposed)
                .unwrap_or_else(DocumentId::generate),
            format: format_of(&path),
            path,
            head_revision_id: DocumentRevisionId::generate(),
            content_sha256: content_hash(&bytes),
            size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            updated_at: Utc::now(),
        };
        self.blobs.insert(entry.content_sha256.clone(), bytes);
        self.note(&entry, kind);
        let revision_id = entry.head_revision_id.clone();
        self.documents.insert(entry.document_id.clone(), entry);
        revision_id
    }

    pub(super) fn cursor(&self) -> String {
        self.changes.len().to_string()
    }

    pub(super) fn held(&self, id: Option<&DocumentId>, path: &str) -> Option<DriveEntry> {
        id.and_then(|id| self.documents.get(id))
            .or_else(|| self.documents.values().find(|entry| entry.path == path))
            .cloned()
    }

    pub(super) fn note(&mut self, entry: &DriveEntry, kind: DriveChangeKind) {
        let deleted = kind == DriveChangeKind::Deleted;
        self.changes.push(DriveChange {
            cursor: (self.changes.len() + 1).to_string(),
            document_id: entry.document_id.clone(),
            path: entry.path.clone(),
            revision_id: (!deleted).then(|| entry.head_revision_id.clone()),
            content_sha256: (!deleted).then(|| entry.content_sha256.clone()),
            kind,
        });
    }
}
