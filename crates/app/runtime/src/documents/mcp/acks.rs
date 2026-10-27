//! Rendering a landed commit's acknowledgement as an MCP edit output.
//!
//! The commit hands back its acknowledgement and the new head's projection,
//! so the written lines render without another read. A replayed
//! acknowledgement carries no projection; its revision is projected once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::service::LandedCommit;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{DocumentUri, Projection};

use super::lines::{MAX_WRITTEN, shows, structured};
use super::outputs::EditOutput;
use super::support::tool_error;
use super::write::EditCall;

fn ack_lines(projection: &Projection, written: &[DocumentNodeId]) -> Vec<String> {
    let written: HashSet<&DocumentNodeId> = written.iter().collect();
    let mut lines = structured(projection, |node| shows(node) && written.contains(&node.id));
    lines.truncate(MAX_WRITTEN);
    lines
}

pub(super) async fn edit_output(
    call: &EditCall<'_>,
    landed: LandedCommit,
) -> Result<EditOutput, McpError> {
    let projection = match landed.projection {
        Some(projection) => projection,
        None => {
            call.tools
                .service()
                .project(
                    call.who,
                    &call.document,
                    Some(&landed.ack.revision_id),
                    call.ctx.trace_id(),
                )
                .await
                .map_err(tool_error)?
                .1
        },
    };
    let ack = landed.ack;
    let verbose = call.verbose;
    Ok(EditOutput {
        uri: verbose
            .then(|| DocumentUri::new(call.document.clone()).at_revision(ack.revision_id.clone())),
        parent_revision_id: ack.parent_revision_id.filter(|_| verbose),
        written: ack_lines(&projection, &ack.written),
        revision_id: ack.revision_id,
        applied: ack.applied,
        merged: ack.merged_from.is_some(),
        replayed: landed.replayed,
        proposal_id: None,
    })
}
