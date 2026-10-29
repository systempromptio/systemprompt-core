//! The live-session events: an engine update, and a checkpoint that saved
//! less than the live document holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCollabUpdateEvent {
    pub document_id: DocumentId,
    pub edit_session_id: DocumentEditSessionId,
    pub seq: i64,
    pub update_b64: String,
    pub author_id: UserId,
}

impl DocumentCollabUpdateEvent {
    pub const NAME: &'static str = "document.collab.update";
}

/// A checkpoint saved placeholders for `images`: uploaded images no
/// inserting editor of the session may place. The live document still
/// holds them, so a later checkpoint saves them once they are admitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCollabDegradedEvent {
    pub document_id: DocumentId,
    pub edit_session_id: DocumentEditSessionId,
    pub images: Vec<String>,
}

impl DocumentCollabDegradedEvent {
    pub const NAME: &'static str = "document.collab.degraded";
}
