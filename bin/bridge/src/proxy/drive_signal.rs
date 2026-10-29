//! Typed signals the proxy hands to the Drive sync loop.
//!
//! The proxy sits below the Drive module, so it cannot call into it. What the
//! proxy observes that Drive cares about — a `PostToolUse` hook naming a file,
//! a `document.revision.created` or `document.presence` frame on the bridge
//! stream — travels over this bounded channel instead. Every signal is a hint:
//! the Drive loop also polls, so a signal dropped on a full channel delays a
//! cycle and loses nothing.
//!
//! A `document.shared` or `document.proposal.opened` frame becomes a
//! [`DocumentNotice`] the desktop shows as a toast; [`notice_for`] is that
//! mapping, kept free of any windowing code so every platform shares it. A
//! share notice is raised only when the frame names this device's user as
//! its recipient.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::Serialize;
use systemprompt_identifiers::{DocumentId, DocumentProposalId, UserId};
use systemprompt_models::documents::{
    DocumentPresenceEvent, DocumentProposalEvent, DocumentRevisionCreatedEvent, DocumentSharedEvent,
};
use tokio::sync::mpsc;

const CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub enum DriveSignal {
    Touched(PathBuf),
    RevisionCreated(Box<DocumentRevisionCreatedEvent>),
    Presence(Box<DocumentPresenceEvent>),
    Notice(Box<DocumentNotice>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    Shared,
    ProposalOpened,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentNotice {
    pub kind: NoticeKind,
    pub document_id: DocumentId,
    pub from: UserId,
    pub from_name: Option<String>,
    pub document_name: Option<String>,
    pub proposal_id: Option<DocumentProposalId>,
    pub detail: Option<String>,
    pub message: Option<String>,
}

#[must_use]
pub fn notice_for(name: &str, value: &str, viewer: Option<&UserId>) -> Option<DocumentNotice> {
    match name {
        DocumentSharedEvent::NAME => {
            let shared: DocumentSharedEvent = decode_frame(name, value)?;
            if viewer != Some(&shared.recipient_user_id) {
                return None;
            }
            Some(DocumentNotice {
                kind: NoticeKind::Shared,
                document_id: shared.document_id,
                from: shared.by,
                from_name: Some(shared.shared_by),
                document_name: Some(shared.document_name),
                proposal_id: None,
                detail: Some(shared.level.to_string()),
                message: shared.message,
            })
        },
        DocumentProposalEvent::OPENED => {
            let opened: DocumentProposalEvent = decode_frame(name, value)?;
            Some(DocumentNotice {
                kind: NoticeKind::ProposalOpened,
                document_id: opened.document_id,
                from: opened.proposer_id,
                from_name: None,
                document_name: None,
                proposal_id: Some(opened.proposal_id),
                detail: opened.message,
                message: None,
            })
        },
        _ => None,
    }
}

fn decode_frame<T: serde::de::DeserializeOwned>(name: &str, value: &str) -> Option<T> {
    match serde_json::from_str(value) {
        Ok(decoded) => Some(decoded),
        Err(error) => {
            tracing::warn!(%error, event = name, "document notice frame did not decode; it is skipped");
            None
        },
    }
}

#[derive(Debug, Clone)]
pub struct DriveSignals {
    tx: mpsc::Sender<DriveSignal>,
}

pub type DriveSignalReceiver = mpsc::Receiver<DriveSignal>;

impl DriveSignals {
    #[must_use]
    pub fn channel() -> (Self, DriveSignalReceiver) {
        let (tx, rx) = mpsc::channel(CAPACITY);
        (Self { tx }, rx)
    }

    pub fn send(&self, signal: DriveSignal) {
        if let Err(error) = self.tx.try_send(signal) {
            tracing::debug!(%error, "drive signal not delivered; the next poll covers it");
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct HookBody {
    #[serde(default)]
    hook_event_name: Option<String>,
    #[serde(default)]
    tool_input: Option<HookToolInput>,
}

#[derive(Debug, serde::Deserialize)]
struct HookToolInput {
    #[serde(default)]
    file_path: Option<PathBuf>,
    #[serde(default)]
    path: Option<PathBuf>,
}

#[must_use]
pub fn touched_by_hook(body: &[u8]) -> Option<PathBuf> {
    let hook: HookBody = serde_json::from_slice(body).ok()?;
    if hook.hook_event_name.as_deref() != Some("PostToolUse") {
        return None;
    }
    let input = hook.tool_input?;
    input.file_path.or(input.path)
}
