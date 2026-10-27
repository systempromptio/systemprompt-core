//! What every handler shares: the caller, the revision author, and the
//! translation of document errors into tool errors an agent can act on.
//!
//! Retriable errors keep the service's Display text, which names the node,
//! hashes, revision or level involved. Infrastructure failures are logged
//! with their cause and answered with a fixed message.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::DocumentRevision;
use systemprompt_files::documents::service::{Author, RevisionOrigin};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, McpExecutionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ActorKind;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_provider_contracts::documents::CodecError;

pub(super) const DEFAULT_LIMIT: i64 = 50;
pub(super) const MAX_LIMIT: i64 = 200;

pub(super) fn caller(ctx: &RequestContext) -> Result<&AuthenticatedUser, McpError> {
    ctx.user.as_ref().ok_or_else(|| {
        McpError::invalid_request(
            "documents tools act as the signed-in user; this call carried no authenticated \
             user",
            None,
        )
    })
}

pub(super) fn agent_author(exec_id: &McpExecutionId) -> Author {
    Author {
        actor_kind: ActorKind::Agent,
        origin: RevisionOrigin::McpExecution(exec_id.clone()),
    }
}

pub(super) fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

#[must_use]
pub fn tool_error(error: DocumentsError) -> McpError {
    match error {
        DocumentsError::Forbidden { .. } => McpError::invalid_request(error.to_string(), None),
        DocumentsError::NotFound { .. } => McpError::resource_not_found(error.to_string(), None),
        DocumentsError::Codec(codec) => codec_error(codec),
        DocumentsError::Conflict { .. }
        | DocumentsError::StaleAnchor { .. }
        | DocumentsError::UnknownNode { .. }
        | DocumentsError::LockHeld { .. }
        | DocumentsError::UnsupportedFormat { .. }
        | DocumentsError::UnknownFormat { .. }
        | DocumentsError::TooLarge { .. }
        | DocumentsError::IntegrityMismatch { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::Template(_) => McpError::invalid_params(error.to_string(), None),
        DocumentsError::Blob(_)
        | DocumentsError::Repository(_)
        | DocumentsError::Authz(_)
        | DocumentsError::Events { .. }
        | DocumentsError::Worker(_) => {
            tracing::error!(
                error = %error,
                cause = ?std::error::Error::source(&error),
                "document tool failed"
            );
            McpError::internal_error("the document store failed; try the call again", None)
        },
    }
}

fn codec_error(error: CodecError) -> McpError {
    match error {
        CodecError::NoMatch => McpError::invalid_params(
            "old_str matches 0 places; read the document text and quote old_str exactly as it \
             appears",
            None,
        ),
        CodecError::AmbiguousMatch { .. }
        | CodecError::UnsupportedOp { .. }
        | CodecError::UnknownNode { .. }
        | CodecError::StaleAnchor { .. }
        | CodecError::Malformed { .. } => McpError::invalid_params(error.to_string(), None),
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
    McpError::invalid_params(
        format!(
            "stale base: document {document} is at head revision {head_id}, written by {author} \
             ({actor}) at {time}, not your base_revision {base}. Read the document again at \
             revision {head_id} and resend the edit with base_revision {head_id}",
            head_id = head.id,
            author = head.author_user_id,
            actor = head.actor_kind,
            time = head.created_at.to_rfc3339(),
        ),
        None,
    )
}
