//! `documents_edit` on a document someone has open live.
//!
//! The batch is written into the open edit session as one engine update
//! (see `CollabService::edit_live`), so every open tab shows it at once and
//! its guards are checked against what those tabs show. The ack names the
//! revision the session is based on, since no revision lands until the
//! session's checkpoint, and keeps `written`: the live lines the batch
//! changed, diffed against the live tree it was applied to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::service::{LiveEdit, LiveEditRequest};
use systemprompt_models::documents::DocumentUri;

use super::support::tool_error;
use super::write::EditCall;
use crate::documents::views::{EditOutput, written_lines};

pub(super) async fn edit_live(
    call: &EditCall<'_>,
    media: &Media,
) -> Result<Option<EditOutput>, McpError> {
    let request = LiveEditRequest {
        document_id: call.document.clone(),
        ops: call.ops.clone(),
        media: media.clone(),
    };
    let live = call
        .tools
        .service()
        .edit_live(call.who, request, call.ctx.trace_id())
        .await
        .map_err(tool_error)?;
    Ok(live.map(|live| live_ack(call, live)))
}

fn live_ack(call: &EditCall<'_>, live: LiveEdit) -> EditOutput {
    EditOutput {
        uri: call.verbose.then(|| {
            DocumentUri::new(call.document.clone()).at_revision(live.base_revision.clone())
        }),
        written: written_lines(Some(&live.before), &live.after),
        revision_id: live.base_revision,
        parent_revision_id: None,
        applied: live.applied,
        merged: false,
        replayed: false,
        proposal_id: None,
    }
}
