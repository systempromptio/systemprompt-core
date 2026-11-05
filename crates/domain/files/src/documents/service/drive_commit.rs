//! Drive commits by path: a known document is committed with the client's
//! base revision; an unknown path in a known folder imports a new document.
//! Folders are never created from a path; an unknown folder is refused.
//! A device acting as an agent capped at `suggest` gets a proposal instead
//! of a commit ([`DriveCommitResponse::Proposed`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    ActorKind, DocumentAction, DriveCommitMeta, DriveCommitResponse, DriveTrashResponse,
    content_hash,
};

use super::support::validate_name;
use super::{
    Author, CommentPolicy, CommitOutcome, CommitRequest, DocumentService, ExternalWrite,
    FolderMove, ImportRequest, UpdateRequest,
};
use crate::documents::error::DocumentsError;
use crate::documents::model::Media;
use crate::documents::models::{LockHolder, RevisionAttribution, RevisionOrigin};
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
            attribution: RevisionAttribution::default(),
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
                    mime_type: None,
                    bytes,
                    author,
                };
                let (_, revision) = self.import_external(who, request, trace).await?;
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
            lock_holder: Some(LockHolder::Bridge(meta.device_id.clone())),
            comments: CommentPolicy::detach(),
            op_id: None,
            media: Media::new(),
        };
        let outcome = match self.write_external(who, request, trace).await? {
            ExternalWrite::Landed(outcome) => outcome,
            ExternalWrite::Proposed(proposed) => {
                return Ok(DriveCommitResponse::Proposed {
                    proposal_id: proposed.proposal.id,
                    revision_id: proposed.revision.id,
                });
            },
        };
        Ok(match outcome {
            CommitOutcome::Committed(landed) => DriveCommitResponse::Committed {
                revision_id: landed.revision.id,
            },
            CommitOutcome::Merged(landed) => DriveCommitResponse::Merged {
                revision_id: landed.revision.id,
                content_sha256: landed.revision.content_sha256,
            },
            CommitOutcome::Conflict { head, base } => DriveCommitResponse::Conflict {
                head_revision_id: head.id,
                head_content_sha256: head.content_sha256,
                base_revision_id: base,
            },
        })
    }

    /// Trashes a document a device removed, unless its head moved past the
    /// revision the device last synced.
    pub async fn drive_trash(
        &self,
        who: &AuthenticatedUser,
        document_id: &DocumentId,
        base: &DocumentRevisionId,
        trace: &TraceId,
    ) -> Result<DriveTrashResponse, DocumentsError> {
        let document = self
            .authorized_document(who, document_id, DocumentAction::Delete, trace)
            .await?;
        let head = self.resolve_revision(&document, None).await?;
        if head.id != *base {
            return Ok(DriveTrashResponse::Conflict {
                head_revision_id: head.id,
                head_content_sha256: head.content_sha256,
            });
        }
        self.trash(who, document_id, trace).await?;
        Ok(DriveTrashResponse::Trashed)
    }

    /// Gives a document the path a device renamed its file to. Folders are
    /// never created from a path; an unknown folder is refused.
    pub async fn drive_move(
        &self,
        who: &AuthenticatedUser,
        document_id: &DocumentId,
        path: &str,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let (folder, name) = self.resolve_drive_path(path).await?;
        let document = self
            .authorized_document(who, document_id, DocumentAction::View, trace)
            .await?;
        let request = UpdateRequest {
            name: (document.name != name).then_some(name),
            folder: if document.folder_id == folder {
                FolderMove::Keep
            } else {
                FolderMove::from_present(folder)
            },
        };
        self.update(who, document_id, request, trace).await?;
        Ok(())
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
