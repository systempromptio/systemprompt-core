//! Edit ops on a workbook or deck session, translated straight into Loro
//! operations under the agent's peer: a workbook through the sheet
//! translator on a fork committed with origin `agent`, a deck through the
//! deck translator, a text document through the rich translator on a fork
//! committed the same way. Each translator names the ids it wrote: block
//! ids for text, `sheet[Name]/A1` cells for a workbook,
//! `slide[N]/shape[S]/p[P]` for a deck. A translator's `Unsupported` answers
//! [`EngineError::Unsupported`], so its caller writes a model instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{ExportMode, LoroDoc};
use systemprompt_models::documents::EditOp;

use super::deck::SLIDES_CONTAINER;
use super::sheet::SHEETS_CONTAINER;
use super::{encode, rich, state};
use crate::documents::collab::ops::deck::{DeckOpError, deck_update};
use crate::documents::collab::ops::rich_loro::{RichOpError, apply_rich_ops_loro};
use crate::documents::collab::ops::sheet::{SheetOpError, apply_sheet_ops};
use crate::documents::engine::{EngineError, OpsUpdate};

pub(super) fn update_from_ops(
    doc: &LoroDoc,
    ops: &[EditOp],
    peer: u64,
) -> Result<Option<OpsUpdate>, EngineError> {
    if !doc.get_movable_list(SHEETS_CONTAINER).is_empty() {
        return on_fork(doc, peer, |draft| {
            apply_sheet_ops(draft, ops).map_err(|error| match error {
                SheetOpError::Unsupported { .. } => EngineError::Unsupported,
                other => EngineError::Model(Box::new(other)),
            })
        });
    }
    if !doc.get_movable_list(SLIDES_CONTAINER).is_empty() {
        let update = deck_update(doc, ops, peer).map_err(|error| match error {
            DeckOpError::Unsupported { .. } => EngineError::Unsupported,
            other => EngineError::Model(Box::new(other)),
        })?;
        return Ok(update.map(|(update, touched)| OpsUpdate { update, touched }));
    }
    on_fork(doc, peer, |draft| {
        apply_rich_ops_loro(draft, ops).map_err(|error| match error {
            RichOpError::Unsupported { .. } | RichOpError::UnknownBlock { .. } => {
                EngineError::Unsupported
            },
            other => EngineError::Model(Box::new(other)),
        })
    })
}

fn on_fork(
    doc: &LoroDoc,
    peer: u64,
    edit: impl FnOnce(&LoroDoc) -> Result<Vec<String>, EngineError>,
) -> Result<Option<OpsUpdate>, EngineError> {
    let before = doc.oplog_vv();
    let draft = doc.fork();
    rich::configure(&draft);
    draft.set_peer_id(peer).map_err(state)?;
    let touched = edit(&draft)?;
    draft.set_next_commit_origin("agent");
    draft.commit();
    if draft.oplog_vv() == before {
        return Ok(None);
    }
    draft
        .export(ExportMode::updates(&before))
        .map(|update| Some(OpsUpdate { update, touched }))
        .map_err(encode)
}
