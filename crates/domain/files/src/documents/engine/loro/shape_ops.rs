//! Edit ops on a workbook or deck session, translated straight into Loro
//! operations under the agent's peer: a workbook through the sheet
//! translator on a fork committed with origin `agent`, a deck through the
//! deck translator. A rich document answers [`EngineError::Unsupported`],
//! so its caller writes a model instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{ExportMode, LoroDoc};
use systemprompt_models::documents::EditOp;

use super::deck::SLIDES_CONTAINER;
use super::sheet::SHEETS_CONTAINER;
use super::{encode, rich, state};
use crate::documents::collab::ops::deck::{DeckOpError, deck_update};
use crate::documents::collab::ops::sheet::{SheetOpError, apply_sheet_ops};
use crate::documents::engine::EngineError;

pub(super) fn update_from_ops(
    doc: &LoroDoc,
    ops: &[EditOp],
    peer: u64,
) -> Result<Option<Vec<u8>>, EngineError> {
    if !doc.get_movable_list(SHEETS_CONTAINER).is_empty() {
        return sheet_update(doc, ops, peer);
    }
    if !doc.get_movable_list(SLIDES_CONTAINER).is_empty() {
        return deck_update(doc, ops, peer).map_err(|error| match error {
            DeckOpError::Unsupported { .. } => EngineError::Unsupported,
            other => EngineError::Model(Box::new(other)),
        });
    }
    Err(EngineError::Unsupported)
}

fn sheet_update(doc: &LoroDoc, ops: &[EditOp], peer: u64) -> Result<Option<Vec<u8>>, EngineError> {
    let before = doc.oplog_vv();
    let draft = doc.fork();
    rich::configure(&draft);
    draft.set_peer_id(peer).map_err(state)?;
    apply_sheet_ops(&draft, ops).map_err(|error| match error {
        SheetOpError::Unsupported { .. } => EngineError::Unsupported,
        other => EngineError::Model(Box::new(other)),
    })?;
    draft.set_next_commit_origin("agent");
    draft.commit();
    if draft.oplog_vv() == before {
        return Ok(None);
    }
    draft
        .export(ExportMode::updates(&before))
        .map(Some)
        .map_err(encode)
}
