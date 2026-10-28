//! The one mapping from a `DocumentsError` to an MCP refusal: every code
//! the tools can answer with, and the typed detail each carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::DocumentsError;
use systemprompt_identifiers::{DocumentCommentId, DocumentNodeId, DocumentRevisionId, UserId};
use systemprompt_models::documents::AccessLevel;
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};

use super::refusal::{Refusal, RefusalCode, refusal};

#[must_use]
pub fn tool_error(error: DocumentsError) -> McpError {
    let message = error.to_string();
    let refusal = match error {
        DocumentsError::Forbidden { owner, .. } => Refusal {
            owner,
            ..Refusal::new(RefusalCode::Forbidden)
        },
        DocumentsError::ShareBelowExisting {
            existing, owner, ..
        } => share_below_existing_refusal(existing, owner),
        DocumentsError::Trashed { .. } => trashed_refusal(),
        DocumentsError::AnchorsAffected { comments } => anchors_affected_refusal(comments),
        DocumentsError::UnknownNode { node } => node_refusal(RefusalCode::NotFound, node),
        DocumentsError::Conflict { head, .. } => Refusal {
            head_revision: Some(head),
            ..Refusal::new(RefusalCode::StaleBase)
        },
        DocumentsError::StaleAnchor {
            node,
            actual,
            current_text,
            ..
        } => Refusal {
            hash: Some(actual),
            current_text: Some(current_text),
            ..node_refusal(RefusalCode::GuardMismatch, node)
        },
        DocumentsError::FindNone { closest, .. } => find_none_refusal(closest),
        DocumentsError::UnsupportedOp { accepted_kinds, .. } => {
            unsupported_op_refusal(accepted_kinds)
        },
        DocumentsError::ProposalConflict {
            head,
            node,
            current_text,
            ..
        } => proposal_conflict_refusal(head, node, current_text),
        DocumentsError::Codec(codec) => return codec_error(&codec),
        DocumentsError::GuardMissing { node, .. } => node_refusal(RefusalCode::InvalidParams, node),
        DocumentsError::Blob(_)
        | DocumentsError::Repository(_)
        | DocumentsError::Authz(_)
        | DocumentsError::Events { .. }
        | DocumentsError::Worker(_)
        | DocumentsError::Directory(_)
        | DocumentsError::LiveSession(_)
        | DocumentsError::UnknownSubject { .. }
        | DocumentsError::NotFound { .. }
        | DocumentsError::CommentNotFound { .. }
        | DocumentsError::LockHeld { .. }
        | DocumentsError::UnsupportedFormat { .. }
        | DocumentsError::UnknownFormat { .. }
        | DocumentsError::Template { .. }
        | DocumentsError::TooLarge { .. }
        | DocumentsError::OpIdMismatch { .. }
        | DocumentsError::SelfReview { .. }
        | DocumentsError::IntegrityMismatch { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::ProposalClosed { .. }
        | DocumentsError::InvalidAwareness { .. }
        | DocumentsError::InvalidComment { .. } => {
            return plain_refusal(&error).map_or_else(
                || internal_failure(&error),
                |code| Refusal::new(code).into_error(message),
            );
        },
    };
    refusal.into_error(message)
}

const fn plain_refusal(error: &DocumentsError) -> Option<RefusalCode> {
    Some(match error {
        DocumentsError::UnknownSubject { .. } => RefusalCode::UnknownSubject,
        DocumentsError::NotFound { .. } | DocumentsError::CommentNotFound { .. } => {
            RefusalCode::NotFound
        },
        DocumentsError::LockHeld { .. } => RefusalCode::Locked,
        DocumentsError::UnsupportedFormat { .. } | DocumentsError::UnknownFormat { .. } => {
            RefusalCode::UnsupportedFormat
        },
        DocumentsError::Template { .. } => RefusalCode::TemplateFailed,
        DocumentsError::TooLarge { .. } => RefusalCode::TooLarge,
        DocumentsError::OpIdMismatch { .. } => RefusalCode::OpIdMismatch,
        DocumentsError::SelfReview { .. } => RefusalCode::SelfReview,
        DocumentsError::IntegrityMismatch { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::ProposalClosed { .. }
        | DocumentsError::InvalidAwareness { .. }
        | DocumentsError::InvalidComment { .. } => RefusalCode::InvalidParams,
        _ => return None,
    })
}

fn trashed_refusal() -> Refusal {
    Refusal {
        trashed: Some(true),
        ..Refusal::new(RefusalCode::NotFound)
    }
}

fn anchors_affected_refusal(comments: Vec<DocumentCommentId>) -> Refusal {
    Refusal {
        comments: Some(comments),
        ..Refusal::new(RefusalCode::AnchorsAffected)
    }
}

fn node_refusal(code: RefusalCode, node: DocumentNodeId) -> Refusal {
    Refusal {
        node: Some(node),
        ..Refusal::new(code)
    }
}

fn codec_error(error: &CodecError) -> McpError {
    match error {
        CodecError::NoMatch => Refusal {
            matches: Some(0),
            ..Refusal::new(RefusalCode::FindNone)
        }
        .into_error(
            "old_str matches 0 places; read the document text and quote old_str exactly as it \
             appears",
        ),
        CodecError::AmbiguousMatch { matches } => Refusal {
            matches: Some(*matches),
            ..Refusal::new(RefusalCode::FindNone)
        }
        .into_error(error.to_string()),
        CodecError::UnsupportedOp { .. } => refusal(RefusalCode::UnsupportedOp, error.to_string()),
        CodecError::UnknownNode { node } => Refusal {
            node: Some(node.clone()),
            ..Refusal::new(RefusalCode::NotFound)
        }
        .into_error(error.to_string()),
        CodecError::StaleAnchor {
            node,
            actual,
            current_text,
            ..
        } => Refusal {
            node: Some(node.clone()),
            hash: Some(actual.clone()),
            current_text: Some(current_text.clone()),
            ..Refusal::new(RefusalCode::GuardMismatch)
        }
        .into_error(error.to_string()),
        CodecError::Malformed { .. } => refusal(RefusalCode::InvalidParams, error.to_string()),
        CodecError::Parse { .. } | CodecError::Io(_) => {
            tracing::error!(
                error = %error,
                cause = ?std::error::Error::source(&error),
                "document codec failed"
            );
            McpError::internal_error("the document could not be processed", None)
        },
    }
}

fn find_none_refusal(closest: Option<ClosestBlock>) -> Refusal {
    let mut refusal = Refusal::new(RefusalCode::FindNone);
    refusal.matches = Some(0);
    if let Some(block) = closest {
        refusal.node = Some(block.node);
        refusal.hash = Some(block.hash);
        refusal.current_text = Some(block.text);
    }
    refusal
}

fn proposal_conflict_refusal(
    head: DocumentRevisionId,
    node: Option<DocumentNodeId>,
    current_text: Option<String>,
) -> Refusal {
    Refusal {
        head_revision: Some(head),
        node,
        current_text,
        ..Refusal::new(RefusalCode::ProposalConflict)
    }
}

fn internal_failure(error: &DocumentsError) -> McpError {
    tracing::error!(
        error = %error,
        cause = ?std::error::Error::source(error),
        "document tool failed"
    );
    McpError::internal_error("the document store failed; try the call again", None)
}

fn share_below_existing_refusal(existing: AccessLevel, owner: Option<UserId>) -> Refusal {
    Refusal {
        owner,
        existing_level: Some(existing),
        ..Refusal::new(RefusalCode::Forbidden)
    }
}

fn unsupported_op_refusal(accepted_kinds: &[&str]) -> Refusal {
    Refusal {
        accepted_kinds: Some(accepted_kinds.iter().map(|k| (*k).to_owned()).collect()),
        ..Refusal::new(RefusalCode::UnsupportedOp)
    }
}
