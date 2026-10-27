//! Stable node id rows and stored write acknowledgements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};

/// One node of a revision: its stable id, the codec path it stands for in
/// that revision, and the node's `hash8`. Entries are kept in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdEntry {
    pub node_id: DocumentNodeId,
    pub path: DocumentNodeId,
    pub hash8: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredWriteAck {
    pub revision_id: DocumentRevisionId,
    // JSON: the caller-facing acknowledgement, stored as written and replayed verbatim.
    pub ack: serde_json::Value,
}
