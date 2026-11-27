//! Structured edits: the request, and whether it landed or left a proposal for
//! a person.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::documents::DriveRevision;
use crate::documents::access::EditMode;
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId};

/// An edit proposal waiting for a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveProposal {
    pub id: DocumentProposalId,
    pub document_id: DocumentId,
    pub base_revision_id: DocumentRevisionId,
    pub proposed_revision_id: DocumentRevisionId,
    pub proposer: String,
    pub actor_kind: String,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub reviewer: Option<String>,
    #[serde(default)]
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub landed_revision_id: Option<DocumentRevisionId>,
    pub created_at: String,
}

/// `GET /{id}/proposals`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveProposals {
    #[serde(default)]
    pub proposals: Vec<DriveProposal>,
}

/// `200` of an edit that landed; `merged_from` when the base was behind and the
/// edit merged onto the head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveLanded {
    pub revision: DriveRevision,
    #[serde(default)]
    pub merged_from: Option<String>,
}

/// `201` of an edit sent with `mode: propose`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveProposed {
    pub proposal: DriveProposal,
    pub revision: DriveRevision,
    #[serde(default)]
    pub superseded: Vec<String>,
}

/// Body of `POST /api/v1/documents/{id}/edits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditRequest {
    // JSON: structured edit ops are the document store's own vocabulary, passed through.
    pub ops: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<EditMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
