//! Awareness: the ephemeral state each participant of a live document
//! shares with the others (who they are, their colour, caret, selection and
//! viewport), and the live channel's own control events.
//!
//! Positions are engine cursors encoded by the client's CRDT engine
//! (`anchor` and `head` are opaque base64 strings tied to a block id), so
//! they survive concurrent edits; the server stores and relays them but
//! never decodes them. An awareness state expires `ttl` after it was sent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, DocumentNodeId, UserId};

pub const AWARENESS_COLOR_MAX: usize = 32;
pub const AWARENESS_CURSOR_MAX: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AwarenessMode {
    Viewing,
    Editing,
    Commenting,
    Presenting,
}

/// A caret or selection: `anchor` and `head` are engine cursors inside the
/// block, cell or shape named by `block`; equal positions mean a caret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AwarenessCursor {
    pub block: DocumentNodeId,
    pub anchor: String,
    pub head: String,
}

/// The visible extent of the participant's canvas, as the first and last
/// visible block (a range of rows or one slide for sheets and decks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AwarenessViewport {
    pub first: DocumentNodeId,
    pub last: DocumentNodeId,
}

/// What a client posts to `POST /documents/{id}/collab/awareness`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AwarenessState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<DocumentEditSessionId>,
    pub color: String,
    pub mode: AwarenessMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<AwarenessCursor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selection: Vec<AwarenessCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<AwarenessViewport>,
    #[schemars(
        description = "Set when the participant is an agent acting for the user, named so a client draws it as its own caret beside the person's."
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentAwarenessEvent {
    pub document_id: DocumentId,
    pub user_id: UserId,
    pub display_name: String,
    pub state: AwarenessState,
    pub expires_at: DateTime<Utc>,
}

impl DocumentAwarenessEvent {
    pub const NAME: &'static str = "document.awareness";
}

/// Why a live channel asks its client to re-join and reload the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LiveResyncReason {
    SessionChanged,
    UpdatesCompacted,
    Lagged,
    RelayLost,
}

/// The first event of every live channel, and of every resumed one.
///
/// `seq` is the last update the open edit session had logged when the
/// channel subscribed; every later update reaches the channel. A client whose
/// editor holds less reconnects with `Last-Event-ID: <session>:<its seq>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentLiveReadyEvent {
    pub document_id: DocumentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_session_id: Option<DocumentEditSessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
    pub replayed: usize,
}

impl DocumentLiveReadyEvent {
    pub const NAME: &'static str = "document.live.ready";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentLiveResyncEvent {
    pub document_id: DocumentId,
    pub reason: LiveResyncReason,
}

impl DocumentLiveResyncEvent {
    pub const NAME: &'static str = "document.live.resync";
}

/// Sent before the server closes a channel whose caller lost view access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentLiveClosedEvent {
    pub document_id: DocumentId,
}

impl DocumentLiveClosedEvent {
    pub const NAME: &'static str = "document.live.closed";
}
