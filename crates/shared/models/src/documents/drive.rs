//! The bridge drive sync protocol.
//!
//! A bridge device mirrors the documents a user can see into a local
//! folder. It fetches a [`DriveManifest`] once, then polls
//! [`DriveChanges`] from the manifest's cursor, downloads blobs by
//! [`ContentHash`], and uploads local edits as a [`DriveCommitMeta`] plus
//! blob. A commit whose `base_revision` is not the current head is merged
//! onto the head when it can be ([`DriveCommitResponse::Merged`] names the
//! merged revision and its content, which the device then pulls) and is
//! otherwise answered with [`DriveCommitResponse::Conflict`] and never
//! applied. [`DriveProposals`] lists the open proposals on the user's
//! documents and the user's own open proposals.
//!
//! The sync runs under its own credential: the device trades its private
//! device token (audience [`BRIDGE_DEVICE_AUDIENCE`], kept by the bridge and
//! never handed to a host) for a [`DriveSyncToken`] (audience
//! [`DRIVE_SYNC_AUDIENCE`]), and the Drive routes accept that token alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContentHash, DocumentId, DocumentProposalId, DocumentRevisionId, DriveDeviceId, UserId,
};
use systemprompt_provider_contracts::documents::DocumentFormat;

use super::access::ActorKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveEntry {
    pub document_id: DocumentId,
    pub path: String,
    pub format: DocumentFormat,
    pub head_revision_id: DocumentRevisionId,
    pub content_sha256: ContentHash,
    pub size_bytes: u64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveManifest {
    pub generated_at: DateTime<Utc>,
    pub cursor: String,
    pub entries: Vec<DriveEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DriveChangeKind {
    Created,
    Updated,
    Renamed { from: String },
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveChange {
    pub cursor: String,
    pub document_id: DocumentId,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<DocumentRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<ContentHash>,
    pub kind: DriveChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveChanges {
    pub changes: Vec<DriveChange>,
    pub next_cursor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveCommitMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<DocumentId>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<DocumentRevisionId>,
    pub content_sha256: ContentHash,
    pub size_bytes: u64,
    pub device_id: DriveDeviceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DriveCommitResponse {
    Committed {
        revision_id: DocumentRevisionId,
    },
    Merged {
        revision_id: DocumentRevisionId,
        content_sha256: ContentHash,
    },
    Conflict {
        head_revision_id: DocumentRevisionId,
        head_content_sha256: ContentHash,
        base_revision_id: Option<DocumentRevisionId>,
    },
    Proposed {
        proposal_id: DocumentProposalId,
        revision_id: DocumentRevisionId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PresenceState {
    Viewing,
    Editing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PresenceSource {
    Bridge,
    Agent,
    Web,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PresenceBeacon {
    pub document_id: DocumentId,
    pub state: PresenceState,
    pub source_kind: PresenceSource,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    // JSON: editor cursor position; the shape belongs to the client's editor.
    pub cursor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveProposal {
    pub proposal_id: DocumentProposalId,
    pub document_id: DocumentId,
    pub proposer_id: UserId,
    pub actor_kind: ActorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveProposals {
    pub proposals: Vec<DriveProposal>,
}

pub const DRIVE_SYNC_AUDIENCE: &str = "bridge.drive_sync";

pub const BRIDGE_DEVICE_AUDIENCE: &str = "bridge.device";

/// The Drive sync's credential, traded for the device's bridge token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriveSyncToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}
