//! `documents_edit` on a document someone has open live.
//!
//! The batch is written into the open edit session as one engine update
//! (see `CollabService::edit_live`), so every open tab shows it at once and
//! its guards are checked against what those tabs show. The ack names the
//! revision the session is based on, since no revision lands until the
//! session's checkpoint, and keeps `written`: the live lines the batch
//! changed, diffed against the live tree it was applied to, or, for a batch
//! the engine translated itself, the live lines of the ids it wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::service::{CommitRequest, LiveEdit};
use systemprompt_models::documents::DocumentUri;

use super::support::tool_error;
use super::write::EditCall;
use crate::documents::views::{self, EditOutput, written_lines};

pub(super) async fn edit_live(
    call: &EditCall<'_>,
    request: &CommitRequest,
) -> Result<Option<EditOutput>, McpError> {
    let live = call
        .tools
        .service()
        .edit_live(call.who, request, call.ctx.trace_id())
        .await
        .map_err(tool_error)?;
    let Some(live) = live else {
        return Ok(None);
    };
    let written = if live.touched.is_empty() {
        written_lines(Some(&live.before), &live.after)
    } else {
        touched_lines(call, &live.touched, &live.values).await?
    };
    Ok(Some(live_ack(call, live, written)))
}

async fn touched_lines(
    call: &EditCall<'_>,
    touched: &[String],
    values: &[(String, String)],
) -> Result<Vec<String>, McpError> {
    let (_, projection) = call
        .tools
        .service()
        .project(call.who, &call.document, None, call.ctx.trace_id())
        .await
        .map_err(tool_error)?;
    Ok(views::touched_lines(&projection, touched, values))
}

fn live_ack(call: &EditCall<'_>, live: LiveEdit, written: Vec<String>) -> EditOutput {
    EditOutput {
        uri: call.verbose.then(|| {
            DocumentUri::new(call.document.clone()).at_revision(live.base_revision.clone())
        }),
        written,
        revision_id: live.base_revision,
        parent_revision_id: None,
        applied: live.applied,
        merged: false,
        replayed: false,
        proposal_id: None,
    }
}
