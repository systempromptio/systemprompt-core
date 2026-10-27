//! What every handler shares: the caller, the revision author, and the
//! translation of document errors into tool errors an agent can act on.
//!
//! Every [`DocumentsError`] maps to one refusal code here. Refusals keep the
//! service's Display text, which names the node, hashes, revision or level
//! involved. Infrastructure failures are logged with their cause and
//! answered with a fixed message as a protocol error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::CallToolResult;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{DocumentRevision, RevisionAttribution};
use systemprompt_files::documents::service::{Author, RevisionOrigin};
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentRevisionId, McpExecutionId, UserId,
};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, ActorKind};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};

use super::DocumentsToolCall;
use super::refusal::{Refusal, RefusalCode, refusal, refusal_result};

pub(super) const DEFAULT_LIMIT: i64 = 50;
pub(super) const MAX_LIMIT: i64 = 200;

pub(super) fn caller(ctx: &RequestContext) -> Result<&AuthenticatedUser, McpError> {
    ctx.user.as_ref().ok_or_else(|| {
        refusal(
            RefusalCode::Forbidden,
            "documents tools act as the signed-in user; this call carried no authenticated user",
        )
    })
}

pub(super) fn agent_author(ctx: &RequestContext, exec_id: &McpExecutionId) -> Author {
    let present = |value: &str| !value.is_empty();
    Author {
        actor_kind: ActorKind::Agent,
        origin: RevisionOrigin::McpExecution(exec_id.clone()),
        attribution: RevisionAttribution {
            session_id: Some(ctx.session_id().clone()).filter(|id| present(id.as_str())),
            context_id: Some(ctx.context_id().clone()).filter(|id| present(id.as_str())),
            trace_id: Some(ctx.trace_id().clone()).filter(|id| present(id.as_str())),
            task_id: ctx.task_id().cloned(),
            on_behalf_of: Some(ctx.user_id().clone()).filter(|id| present(id.as_str())),
        },
    }
}

pub(super) fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

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
        DocumentsError::UnknownSubject { .. } => Refusal::new(RefusalCode::UnknownSubject),
        DocumentsError::NotFound { .. } | DocumentsError::CommentNotFound { .. } => {
            Refusal::new(RefusalCode::NotFound)
        },
        DocumentsError::Trashed { .. } => Refusal {
            trashed: Some(true),
            ..Refusal::new(RefusalCode::NotFound)
        },
        DocumentsError::AnchorsAffected { comments } => Refusal {
            comments: Some(comments),
            ..Refusal::new(RefusalCode::AnchorsAffected)
        },
        DocumentsError::UnknownNode { node } => Refusal {
            node: Some(node),
            ..Refusal::new(RefusalCode::NotFound)
        },
        DocumentsError::LockHeld { .. } => Refusal::new(RefusalCode::Locked),
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
            node: Some(node),
            hash: Some(actual),
            current_text: Some(current_text),
            ..Refusal::new(RefusalCode::GuardMismatch)
        },
        DocumentsError::FindNone { closest, .. } => find_none_refusal(closest),
        DocumentsError::UnsupportedOp { accepted_kinds, .. } => Refusal {
            accepted_kinds: Some(accepted_kinds.iter().map(|k| (*k).to_owned()).collect()),
            ..Refusal::new(RefusalCode::UnsupportedOp)
        },
        DocumentsError::UnsupportedFormat { .. } | DocumentsError::UnknownFormat { .. } => {
            Refusal::new(RefusalCode::UnsupportedFormat)
        },
        DocumentsError::Template { .. } => Refusal::new(RefusalCode::TemplateFailed),
        DocumentsError::ProposalConflict {
            head,
            node,
            current_text,
            ..
        } => proposal_conflict_refusal(head, node, current_text),
        DocumentsError::Codec(codec) => return codec_error(&codec),
        DocumentsError::TooLarge { .. }
        | DocumentsError::IntegrityMismatch { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::ProposalClosed { .. }
        | DocumentsError::InvalidComment { .. } => Refusal::new(RefusalCode::InvalidParams),
        DocumentsError::Blob(_)
        | DocumentsError::Repository(_)
        | DocumentsError::Authz(_)
        | DocumentsError::Events { .. }
        | DocumentsError::Worker(_)
        | DocumentsError::Directory(_) => return internal_failure(&error),
    };
    refusal.into_error(message)
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

pub(super) fn stale_base(
    document: &DocumentId,
    head: &DocumentRevision,
    base: Option<&DocumentRevisionId>,
) -> McpError {
    let base = base.map_or_else(|| "none".to_owned(), ToString::to_string);
    Refusal {
        head_revision: Some(head.id.clone()),
        written_by: Some(head.author_user_id.clone()),
        actor_kind: Some(head.actor_kind),
        at: Some(head.created_at),
        ..Refusal::new(RefusalCode::StaleBase)
    }
    .into_error(format!(
        "stale base: document {document} is at head revision {head_id}, written by {author} \
         ({actor}) at {time}, not your base_revision {base}. Their edit wins: read revision \
         {head_id}, re-aim your change at the text as it stands and never resend your old text \
         over it",
        head_id = head.id,
        author = head.author_user_id,
        actor = head.actor_kind,
        time = head.created_at.to_rfc3339(),
    ))
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

pub(super) async fn run<H: McpToolHandler>(
    call: DocumentsToolCall<'_>,
    handler: &H,
) -> Result<CallToolResult, McpError> {
    match call
        .executor
        .execute(handler, call.request, call.context, call.client)
        .await
    {
        Err(error) => refusal_result(&error).ok_or(error),
        ok => ok,
    }
}

fn share_below_existing_refusal(existing: AccessLevel, owner: Option<UserId>) -> Refusal {
    Refusal {
        owner,
        existing_level: Some(existing),
        ..Refusal::new(RefusalCode::Forbidden)
    }
}
