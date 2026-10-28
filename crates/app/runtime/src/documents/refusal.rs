//! Refusals: the one vocabulary both document surfaces answer a client
//! error in.
//!
//! [`refusal_of`] maps every [`DocumentsError`] a client can act on to one
//! [`RefusalCode`] and the typed detail the failure knows (the head revision
//! and who wrote it, a node's current text, the kinds a format accepts),
//! with the service's Display text as the message. An infrastructure failure
//! maps to none: each surface logs its cause and answers a fixed message.
//! The MCP tools return the [`Refusal`] as a refused tool result's
//! `structuredContent`; the HTTP routes return its code as the envelope's
//! `error` key and the refusal under `detail`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::DocumentRevision;
use systemprompt_identifiers::{
    ContentHash, DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, UserId,
};
use systemprompt_models::documents::{AccessLevel, ActorKind};
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};

/// The stable code of a refusal; `documents_guide` explains each as
/// `refusal.<code>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    StaleBase,
    GuardMismatch,
    FindNone,
    UnsupportedOp,
    UnsupportedFormat,
    TemplateFailed,
    Forbidden,
    NotFound,
    Trashed,
    Locked,
    FolderNotEmpty,
    AnchorsAffected,
    ProposalConflict,
    ProposalClosed,
    InvalidParams,
    UnknownSubject,
    TooLarge,
    IntegrityMismatch,
    OpIdMismatch,
    SelfReview,
    AccessRequestOpen,
    AccessRequestNotNeeded,
    AccessRequestClosed,
}

impl RefusalCode {
    pub const ALL: [Self; 23] = [
        Self::StaleBase,
        Self::GuardMismatch,
        Self::FindNone,
        Self::UnsupportedOp,
        Self::UnsupportedFormat,
        Self::TemplateFailed,
        Self::Forbidden,
        Self::NotFound,
        Self::Trashed,
        Self::Locked,
        Self::FolderNotEmpty,
        Self::AnchorsAffected,
        Self::ProposalConflict,
        Self::ProposalClosed,
        Self::InvalidParams,
        Self::UnknownSubject,
        Self::TooLarge,
        Self::IntegrityMismatch,
        Self::OpIdMismatch,
        Self::SelfReview,
        Self::AccessRequestOpen,
        Self::AccessRequestNotNeeded,
        Self::AccessRequestClosed,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleBase => "stale_base",
            Self::GuardMismatch => "guard_mismatch",
            Self::FindNone => "find_none",
            Self::UnsupportedOp => "unsupported_op",
            Self::UnsupportedFormat => "unsupported_format",
            Self::TemplateFailed => "template_failed",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Trashed => "trashed",
            Self::Locked => "locked",
            Self::FolderNotEmpty => "folder_not_empty",
            Self::AnchorsAffected => "anchors_affected",
            Self::ProposalConflict => "proposal_conflict",
            Self::ProposalClosed => "proposal_closed",
            Self::InvalidParams => "invalid_params",
            Self::UnknownSubject => "unknown_subject",
            Self::TooLarge => "too_large",
            Self::IntegrityMismatch => "integrity_mismatch",
            Self::OpIdMismatch => "op_id_mismatch",
            Self::SelfReview => "self_review",
            Self::AccessRequestOpen => "access_request_open",
            Self::AccessRequestNotNeeded => "access_request_not_needed",
            Self::AccessRequestClosed => "access_request_closed",
        }
    }

    #[must_use]
    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == code)
    }
}

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
    let code = |code| Some(Refusal::new(code));
    match error {
        DocumentsError::Conflict { head, base, .. } => Some(Refusal {
            head_revision: Some(head.clone()),
            base_revision: base.clone(),
            ..Refusal::new(RefusalCode::StaleBase)
        }),
        DocumentsError::StaleAnchor {
            node,
            actual,
            current_text,
            ..
        } => Some(guard_mismatch(node, actual, current_text)),
        DocumentsError::Forbidden { owner, .. } => Some(Refusal {
            owner: owner.clone(),
            ..Refusal::new(RefusalCode::Forbidden)
        }),
        DocumentsError::ShareBelowExisting {
            existing, owner, ..
        } => Some(Refusal {
            owner: owner.clone(),
            existing_level: Some(*existing),
            ..Refusal::new(RefusalCode::Forbidden)
        }),
        DocumentsError::UnknownNode { node } | DocumentsError::GuardMissing { node, .. } => {
            Some(Refusal {
                node: Some(node.clone()),
                ..Refusal::new(node_code(error))
            })
        },
        DocumentsError::Trashed { .. } => Some(Refusal {
            trashed: Some(true),
            ..Refusal::new(RefusalCode::Trashed)
        }),
        DocumentsError::AnchorsAffected { comments } => Some(Refusal {
            comments: Some(comments.clone()),
            ..Refusal::new(RefusalCode::AnchorsAffected)
        }),
        DocumentsError::FindNone { closest, .. } => Some(find_none(closest.as_ref())),
        DocumentsError::UnsupportedOp { accepted_kinds, .. } => Some(Refusal {
            accepted_kinds: Some(accepted_kinds.iter().map(|k| (*k).to_owned()).collect()),
            ..Refusal::new(RefusalCode::UnsupportedOp)
        }),
        DocumentsError::ProposalConflict {
            head,
            node,
            current_text,
            ..
        } => Some(Refusal {
            head_revision: Some(head.clone()),
            node: node.clone(),
            current_text: current_text.clone(),
            ..Refusal::new(RefusalCode::ProposalConflict)
        }),
        DocumentsError::NotFound { .. } | DocumentsError::CommentNotFound { .. } => {
            code(RefusalCode::NotFound)
        },
        DocumentsError::UnknownSubject { .. } => code(RefusalCode::UnknownSubject),
        DocumentsError::LockHeld { .. } => code(RefusalCode::Locked),
        DocumentsError::FolderNotEmpty { .. } => code(RefusalCode::FolderNotEmpty),
        DocumentsError::ProposalClosed { .. } => code(RefusalCode::ProposalClosed),
        DocumentsError::IntegrityMismatch { .. } => code(RefusalCode::IntegrityMismatch),
        DocumentsError::UnsupportedFormat { .. } | DocumentsError::UnknownFormat { .. } => {
            code(RefusalCode::UnsupportedFormat)
        },
        DocumentsError::Template { .. } => code(RefusalCode::TemplateFailed),
        DocumentsError::TooLarge { .. } => code(RefusalCode::TooLarge),
        DocumentsError::OpIdMismatch { .. } => code(RefusalCode::OpIdMismatch),
        DocumentsError::SelfReview { .. } => code(RefusalCode::SelfReview),
        DocumentsError::AccessRequestOpen { .. } => code(RefusalCode::AccessRequestOpen),
        DocumentsError::AccessRequestNotNeeded { .. } => code(RefusalCode::AccessRequestNotNeeded),
        DocumentsError::AccessRequestClosed { .. } => code(RefusalCode::AccessRequestClosed),
        DocumentsError::InvalidParams { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::InvalidComment { .. }
        | DocumentsError::InvalidAwareness { .. } => code(RefusalCode::InvalidParams),
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

const fn node_code(error: &DocumentsError) -> RefusalCode {
    if matches!(error, DocumentsError::GuardMissing { .. }) {
        RefusalCode::InvalidParams
    } else {
        RefusalCode::NotFound
    }
}

fn guard_mismatch(node: &DocumentNodeId, actual: &str, current_text: &str) -> Refusal {
    Refusal {
        node: Some(node.clone()),
        hash: Some(actual.to_owned()),
        current_text: Some(current_text.to_owned()),
        ..Refusal::new(RefusalCode::GuardMismatch)
    }
}

fn find_none(closest: Option<&ClosestBlock>) -> Refusal {
    let mut refusal = Refusal::new(RefusalCode::FindNone);
    refusal.matches = Some(0);
    if let Some(block) = closest {
        refusal.node = Some(block.node.clone());
        refusal.hash = Some(block.hash.clone());
        refusal.current_text = Some(block.text.clone());
    }
    refusal
}

fn codec_refusal(error: &CodecError) -> Option<Refused> {
    let message = error.to_string();
    let refusal = match error {
        CodecError::NoMatch => {
            return Some(
                Refusal {
                    matches: Some(0),
                    ..Refusal::new(RefusalCode::FindNone)
                }
                .with_message(
                    "old_str matches 0 places; read the document text and quote old_str exactly \
                     as it appears",
                ),
            );
        },
        CodecError::AmbiguousMatch { matches } => Refusal {
            matches: Some(*matches),
            ..Refusal::new(RefusalCode::FindNone)
        },
        CodecError::UnsupportedOp { .. } => Refusal::new(RefusalCode::UnsupportedOp),
        CodecError::UnknownNode { node } => Refusal {
            node: Some(node.clone()),
            ..Refusal::new(RefusalCode::NotFound)
        },
        CodecError::StaleAnchor {
            node,
            actual,
            current_text,
            ..
        } => guard_mismatch(node, actual, current_text),
        CodecError::Malformed { .. } | CodecError::Parse { .. } => {
            Refusal::new(RefusalCode::InvalidParams)
        },
        CodecError::Io(_) => return None,
    };
    Some(refusal.with_message(message))
}
