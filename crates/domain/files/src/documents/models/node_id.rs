//! Stable node id rows and stored write acknowledgements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentOpId, DocumentRevisionId, UserId,
};

/// One node of a revision: its stable id, the codec path it stands for in
/// that revision, and the node's `hash8`. Entries are kept in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdEntry {
    pub node_id: DocumentNodeId,
    pub path: DocumentNodeId,
    pub hash8: String,
}

/// The stored acknowledgement of a landed commit that carried an op id.
///
/// It is written in the commit's transaction and answered verbatim to a
/// retry of that op. `written` names the nodes whose content the commit wrote
/// (diffed by hash against the parent); `created` the ids it minted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteAck {
    pub revision_id: DocumentRevisionId,
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub merged_from: Option<DocumentRevisionId>,
    pub applied: usize,
    pub written: Vec<DocumentNodeId>,
    pub created: Vec<DocumentNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWriteAck {
    pub user_id: Option<UserId>,
    pub request_sha256: Option<String>,
    pub ack: WriteAck,
}

#[derive(Debug, Clone, Copy)]
pub struct AckClaim<'a> {
    pub document: &'a DocumentId,
    pub op_id: &'a DocumentOpId,
    pub user: &'a UserId,
    pub request_sha256: &'a str,
    pub ack: &'a WriteAck,
}
