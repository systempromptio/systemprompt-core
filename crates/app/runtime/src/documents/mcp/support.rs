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
use systemprompt_files::documents::models::{DocumentRevision, RevisionAttribution};
use systemprompt_files::documents::service::{Author, RevisionOrigin};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ActorKind;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsToolCall;
use super::refusal::{Refusal, RefusalCode, refusal, refusal_result};
pub use super::tool_error::tool_error;

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
