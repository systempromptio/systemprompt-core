//! A landed commit's acknowledgement as the lines it wrote.
//!
//! The commit hands back its acknowledgement and the new head's projection,
//! so the written lines render from the projection the write built: one
//! parse per write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_files::documents::service::LandedCommit;
use systemprompt_identifiers::{DocumentId, DocumentNodeId};
use systemprompt_models::documents::{DocumentUri, Projection};

use super::lines::{MAX_WRITTEN, shows, structured};
use super::types::EditOutput;

fn ack_lines(projection: &Projection, written: &[DocumentNodeId]) -> Vec<String> {
    let written: HashSet<&DocumentNodeId> = written.iter().collect();
    let mut lines = structured(projection, |node| shows(node) && written.contains(&node.id));
    lines.truncate(MAX_WRITTEN);
    lines
}

#[must_use]
pub fn edit_ack(document: &DocumentId, landed: LandedCommit, verbose: bool) -> EditOutput {
    let ack = landed.ack;
    EditOutput {
        uri: verbose
            .then(|| DocumentUri::new(document.clone()).at_revision(ack.revision_id.clone())),
        parent_revision_id: ack.parent_revision_id.filter(|_| verbose),
        written: ack_lines(&landed.projection, &ack.written),
        revision_id: ack.revision_id,
        applied: ack.applied,
        merged: ack.merged_from.is_some(),
        replayed: landed.replayed,
        proposal_id: None,
    }
}
