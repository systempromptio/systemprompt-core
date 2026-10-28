//! Refusals: the one vocabulary both document surfaces answer a client
//! error in.
//!
//! `refusal_of` maps every `DocumentsError` a client can act on to one
//! `RefusalCode` and the typed detail the failure knows (the head revision
//! and who wrote it, a node's current text, the kinds a format accepts),
//! with the service's Display text as the message. An infrastructure failure
//! maps to none: each surface logs its cause and answers a fixed message.
//! The MCP tools return the `Refusal` as a refused tool result's
//! `structuredContent`; the HTTP routes return its code as the envelope's
//! `error` key and the refusal under `detail`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod code;
mod codec;

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::AgentRefusal;
use systemprompt_files::documents::models::DocumentRevision;
use systemprompt_identifiers::{
    ContentHash, DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, UserId,
};
use systemprompt_models::documents::{AccessLevel, ActorKind};

pub use code::RefusalCode;

use self::codec::{codec_refusal, find_none, guard_mismatch};

/// A refusal's code and typed detail: the MCP `structuredContent` of a
/// refused tool call and the HTTP envelope's `detail`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    pub code: RefusalCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_revision: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_content_sha256: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<DocumentRevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written_by: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_kind: Option<ActorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<DocumentNodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_kinds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_level: Option<AccessLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<DocumentCommentId>>,
}

/// A refusal and the message a client reads beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub refusal: Refusal,
    pub message: String,
}

impl Refusal {
    #[must_use]
    pub const fn new(code: RefusalCode) -> Self {
        Self {
            code,
            head_revision: None,
            head_content_sha256: None,
            base_revision: None,
            written_by: None,
            actor_kind: None,
            at: None,
            node: None,
            hash: None,
            current_text: None,
            matches: None,
            accepted_kinds: None,
            owner: None,
            existing_level: None,
            trashed: None,
            comments: None,
        }
    }

    #[must_use]
    pub fn with_message(self, message: impl Into<String>) -> Refused {
        Refused {
            refusal: self,
            message: message.into(),
        }
    }
}

#[must_use]
pub fn stale_base(
    document: &DocumentId,
    head: &DocumentRevision,
    base: Option<&DocumentRevisionId>,
) -> Refused {
    let base_label = base.map_or_else(|| "none".to_owned(), ToString::to_string);
    let message = format!(
        "stale base: document {document} is at head revision {head_id}, written by {author} \
         ({actor}) at {time}, not your base_revision {base_label}. Their edit wins: read \
         revision {head_id}, re-aim your change at the text as it stands and never resend your \
         old text over it",
        head_id = head.id,
        author = head.author_user_id,
        actor = head.actor_kind,
        time = head.created_at.to_rfc3339(),
    );
    Refusal {
        head_revision: Some(head.id.clone()),
        head_content_sha256: Some(head.content_sha256.clone()),
        base_revision: base.cloned(),
        written_by: Some(head.author_user_id.clone()),
        actor_kind: Some(head.actor_kind),
        at: Some(head.created_at),
        ..Refusal::new(RefusalCode::StaleBase)
    }
    .with_message(message)
}

#[must_use]
pub fn refusal_of(error: &DocumentsError) -> Option<Refused> {
    if let DocumentsError::Codec(codec) = error {
        return codec_refusal(codec);
    }
    Some(service_refusal(error)?.with_message(error.to_string()))
}

fn service_refusal(error: &DocumentsError) -> Option<Refusal> {
    let base = Refusal::new(plain_code(error)?);
    Some(match error {
        DocumentsError::Conflict {
            head, base: sent, ..
        } => Refusal {
            head_revision: Some(head.clone()),
            base_revision: sent.clone(),
            ..base
        },
        DocumentsError::StaleAnchor {
            node,
            actual,
            current_text,
            ..
        } => guard_mismatch(node, actual, current_text),
        DocumentsError::Forbidden { owner, .. } => Refusal {
            owner: owner.clone(),
            ..base
        },
        DocumentsError::ShareBelowExisting {
            existing, owner, ..
        } => Refusal {
            owner: owner.clone(),
            existing_level: Some(*existing),
            ..base
        },
        DocumentsError::UnknownNode { node } | DocumentsError::GuardMissing { node, .. } => {
            Refusal {
                node: Some(node.clone()),
                ..base
            }
        },
        DocumentsError::Trashed { .. } => Refusal {
            trashed: Some(true),
            ..base
        },
        DocumentsError::AnchorsAffected { comments } => Refusal {
            comments: Some(comments.clone()),
            ..base
        },
        DocumentsError::FindNone { closest, .. } => find_none(closest.as_ref()),
        DocumentsError::UnsupportedOp { accepted_kinds, .. } => Refusal {
            accepted_kinds: Some(accepted_kinds.iter().map(|k| (*k).to_owned()).collect()),
            ..base
        },
        DocumentsError::ProposalConflict {
            head,
            node,
            current_text,
            ..
        } => Refusal {
            head_revision: Some(head.clone()),
            node: node.clone(),
            current_text: current_text.clone(),
            ..base
        },
        _ => base,
    })
}

const fn agent_code(refusal: &AgentRefusal) -> RefusalCode {
    match refusal {
        AgentRefusal::CapExceeded { .. } | AgentRefusal::HardRule { .. } => {
            RefusalCode::AgentForbidden
        },
        AgentRefusal::SuggestOnly { .. } => RefusalCode::AgentSuggestOnly,
        AgentRefusal::RateLimited { .. } => RefusalCode::AgentRateLimited,
        AgentRefusal::OverrideLooser { .. } => RefusalCode::InvalidParams,
        AgentRefusal::MoveRefused { .. } => RefusalCode::Forbidden,
    }
}

const fn plain_code(error: &DocumentsError) -> Option<RefusalCode> {
    match error {
        DocumentsError::Conflict { .. } => Some(RefusalCode::StaleBase),
        DocumentsError::StaleAnchor { .. } => Some(RefusalCode::GuardMismatch),
        DocumentsError::Forbidden { .. } | DocumentsError::ShareBelowExisting { .. } => {
            Some(RefusalCode::Forbidden)
        },
        DocumentsError::Trashed { .. } => Some(RefusalCode::Trashed),
        DocumentsError::AnchorsAffected { .. } => Some(RefusalCode::AnchorsAffected),
        DocumentsError::FindNone { .. } => Some(RefusalCode::FindNone),
        DocumentsError::UnsupportedOp { .. } => Some(RefusalCode::UnsupportedOp),
        DocumentsError::ProposalConflict { .. } => Some(RefusalCode::ProposalConflict),
        DocumentsError::UnknownNode { .. }
        | DocumentsError::NotFound { .. }
        | DocumentsError::CommentNotFound { .. } => Some(RefusalCode::NotFound),
        DocumentsError::UnknownSubject { .. } => Some(RefusalCode::UnknownSubject),
        DocumentsError::LockHeld { .. } => Some(RefusalCode::Locked),
        DocumentsError::FolderNotEmpty { .. } => Some(RefusalCode::FolderNotEmpty),
        DocumentsError::ProposalClosed { .. } => Some(RefusalCode::ProposalClosed),
        DocumentsError::IntegrityMismatch { .. } => Some(RefusalCode::IntegrityMismatch),
        DocumentsError::UnsupportedFormat { .. } | DocumentsError::UnknownFormat { .. } => {
            Some(RefusalCode::UnsupportedFormat)
        },
        DocumentsError::Template { .. } => Some(RefusalCode::TemplateFailed),
        DocumentsError::TooLarge { .. } => Some(RefusalCode::TooLarge),
        DocumentsError::OpIdMismatch { .. } => Some(RefusalCode::OpIdMismatch),
        DocumentsError::SelfReview { .. } => Some(RefusalCode::SelfReview),
        DocumentsError::AccessRequestOpen { .. } => Some(RefusalCode::AccessRequestOpen),
        DocumentsError::AccessRequestNotNeeded { .. } => Some(RefusalCode::AccessRequestNotNeeded),
        DocumentsError::AccessRequestClosed { .. } => Some(RefusalCode::AccessRequestClosed),
        DocumentsError::Agent(refusal) => Some(agent_code(refusal)),
        DocumentsError::InvalidParams { .. }
        | DocumentsError::ContentRefused { .. }
        | DocumentsError::GuardMissing { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::InvalidComment { .. }
        | DocumentsError::InvalidAwareness { .. } => Some(RefusalCode::InvalidParams),
        DocumentsError::Codec(_)
        | DocumentsError::Blob(_)
        | DocumentsError::Repository(_)
        | DocumentsError::Authz(_)
        | DocumentsError::Events { .. }
        | DocumentsError::Worker(_)
        | DocumentsError::Directory(_)
        | DocumentsError::LiveSession(_) => None,
    }
}
