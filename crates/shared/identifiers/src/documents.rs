//! Shared document store identifiers.
//!
//! Documents, folders, revisions, edit sessions, comments, proposals and
//! access requests, resumable uploads and issue reports, are UUID-backed.
//! [`ContentHash`] is a lowercase hex SHA-256 digest: the blob address a
//! revision, a drive manifest entry and a bridge commit all carry, so it is
//! checked to exactly 64 lowercase hex characters. The digest itself is
//! computed in `systemprompt_provider_contracts::documents::content_hash`,
//! keeping this crate free of hashing dependencies.
//!
//! [`DocumentNodeId`] is the stable anchor of one node of a document
//! projection, minted once and carried across revisions;
//! [`DocumentOpId`] is the caller-chosen key that makes a write idempotent;
//! [`DocumentTemplateId`] names a registered template; [`DriveDeviceId`]
//! names the bridge device that committed a revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

crate::define_id!(DocumentId, uuid);
crate::define_id!(DocumentFolderId, uuid);
crate::define_id!(DocumentRevisionId, uuid);
crate::define_id!(DocumentEditSessionId, uuid);
crate::define_id!(DocumentCommentId, uuid);
crate::define_id!(DocumentProposalId, uuid);
crate::define_id!(DocumentAccessRequestId, uuid);
crate::define_id!(DocumentUploadId, uuid);
crate::define_id!(IssueReportId, uuid);
crate::define_id!(ContentHash, checked, validate_content_hash);
crate::define_id!(DocumentNodeId, checked, |value| {
    crate::macros::validate_non_empty("DocumentNodeId", value)
});
crate::define_id!(DocumentOpId, checked, |value| {
    crate::macros::validate_non_empty("DocumentOpId", value)
});
crate::define_id!(DocumentTemplateId, checked, |value| {
    crate::macros::validate_non_empty("DocumentTemplateId", value)
});
crate::define_id!(DriveDeviceId, checked, |value| {
    crate::macros::validate_non_empty("DriveDeviceId", value)
});

const CONTENT_HASH_LEN: usize = 64;

fn validate_content_hash(value: &str) -> Result<(), IdValidationError> {
    if value.is_empty() {
        return Err(IdValidationError::empty("ContentHash"));
    }
    if value.len() != CONTENT_HASH_LEN {
        return Err(IdValidationError::invalid(
            "ContentHash",
            "must be exactly 64 hex characters",
        ));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(IdValidationError::invalid(
            "ContentHash",
            "must be lowercase hex",
        ));
    }
    Ok(())
}
