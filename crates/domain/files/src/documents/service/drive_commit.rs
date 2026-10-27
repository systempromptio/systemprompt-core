//! Drive commits by path: a known document is committed with the client's
//! base revision; an unknown path in a known folder imports a new document.
//! Folders are never created from a path; an unknown folder is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    ActorKind, DriveCommitMeta, DriveCommitResponse, content_hash,
};

use super::support::validate_name;
use super::{Author, CommitOutcome, CommitRequest, DocumentService, ImportRequest, RevisionOrigin};
use crate::documents::error::DocumentsError;
use crate::documents::revision::CommitBody;

impl DocumentService {
    pub async fn drive_commit(
        &self,
        who: &AuthenticatedUser,
        meta: DriveCommitMeta,
        bytes: Vec<u8>,
        trace: &TraceId,
    ) -> Result<DriveCommitResponse, DocumentsError> {
        let actual = content_hash(&bytes);
        if actual != meta.content_sha256 || bytes.len() as u64 != meta.size_bytes {
            return Err(DocumentsError::IntegrityMismatch {
                declared: meta.content_sha256,
                actual,
            });
        }
        let author = Author {
            actor_kind: ActorKind::Bridge,
            origin: RevisionOrigin::BridgeDevice(meta.device_id.clone()),
        };
        let document_id = if let Some(id) = meta.document_id {
            id
        } else {
            let (folder, name) = self.resolve_drive_path(&meta.path).await?;
            let existing = self
                .repos
                .documents
                .find_in_folder(folder.as_ref(), &name)
                .await?;
            if let Some(existing) = existing {
                existing.id
            } else {
                let request = ImportRequest {
                    folder,
                    name,
                    format: None,
                    bytes,
                    author,
                };
                let (_, revision) = self.import(who, request, trace).await?;
                return Ok(DriveCommitResponse::Committed {
                    revision_id: revision.id,
                });
            }
        };
        let request = CommitRequest {
            document_id,
            base_revision: meta.base_revision,
            body: CommitBody::Bytes(bytes),
            author,
            message: meta.message,
            lock_holder_id: Some(meta.device_id.as_str().to_owned()),
        };
        Ok(match self.commit(who, request, trace).await? {
            CommitOutcome::Committed { revision, .. } | CommitOutcome::Merged { revision, .. } => {
                DriveCommitResponse::Committed {
                    revision_id: revision.id,
                }
            },
            CommitOutcome::Conflict { head, base } => DriveCommitResponse::Conflict {
                head_revision_id: head.id,
                head_content_sha256: head.content_sha256,
                base_revision_id: base,
            },
        })
    }

    async fn resolve_drive_path(
        &self,
        path: &str,
    ) -> Result<(Option<DocumentFolderId>, String), DocumentsError> {
        let mut segments: Vec<&str> = path.split('/').collect();
        let name = segments.pop().unwrap_or_default().to_owned();
        validate_name(&name)?;
        let mut folder: Option<DocumentFolderId> = None;
        for (depth, segment) in segments.iter().enumerate() {
            validate_name(segment)?;
            let child = self
                .repos
                .folders
                .list_children(folder.as_ref())
                .await?
                .into_iter()
                .find(|child| child.name == *segment)
                .ok_or_else(|| DocumentsError::not_found("folder", segments[..=depth].join("/")))?;
            folder = Some(child.id);
        }
        Ok((folder, name))
    }
}
