//! What every handler shares: the caller, the revision author, and the
//! translation of document errors into tool errors an agent can act on.
//!
//! Every `DocumentsError` maps to one refusal code through
//! `refusal_of`, shared with the HTTP routes. Infrastructure failures are
//! logged with their cause and answered with a fixed message as a protocol
//! error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::model::CallToolResult;
use systemprompt_extension::mcp::{McpToolHandler, execute};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{DocumentRevision, RevisionOrigin};
use systemprompt_files::documents::service::Author;
use systemprompt_identifiers::{ContextId, DocumentId, DocumentRevisionId, McpExecutionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ActorKind;
use systemprompt_models::execution::context::RequestContext;

use super::refusal::{refusal, refusal_result, refused_error};
use super::{DocumentsMcpTools, DocumentsToolCall};
use crate::documents::attribution::request_author;
use crate::documents::refusal::{RefusalCode, refusal_of, stale_base as stale_base_refusal};

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

pub(super) async fn agent_author(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    exec_id: &McpExecutionId,
    context_id: Option<&ContextId>,
) -> Result<Author, McpError> {
    request_author(
        tools.contexts().as_ref(),
        ctx,
        (
            ActorKind::Agent,
            RevisionOrigin::McpExecution(exec_id.clone()),
        ),
        context_id.or_else(|| Some(ctx.context_id())),
    )
    .await
    .map_err(tool_error)
}

pub(super) fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "handlers pass it as the fn pointer of map_err over an owned DocumentsError"
)]
pub fn tool_error(error: DocumentsError) -> McpError {
    refusal_of(&error).map_or_else(|| internal_failure(&error), refused_error)
}

pub(super) fn stale_base(
    document: &DocumentId,
    head: &DocumentRevision,
    base: Option<&DocumentRevisionId>,
) -> McpError {
    refused_error(stale_base_refusal(document, head, base))
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
    match execute(
        call.executor,
        handler,
        call.request,
        call.context,
        call.client,
    )
    .await
    {
        Err(error) => refusal_result(&error).ok_or(error),
        ok => ok,
    }
}
