//! The in-memory Drive: documents at their head revision, the blobs they
//! name, and the change feed every write appends to.

use std::collections::{BTreeMap, HashMap};

use systemprompt_identifiers::{ContentHash, DocumentId};
use systemprompt_models::documents::{DriveChange, DriveChangeKind, DriveEntry};

#[derive(Debug, Default)]
pub(crate) struct DriveStore {
    pub(super) documents: BTreeMap<DocumentId, DriveEntry>,
    pub(super) blobs: HashMap<ContentHash, Vec<u8>>,
    pub(super) changes: Vec<DriveChange>,
}

impl DriveStore {
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
