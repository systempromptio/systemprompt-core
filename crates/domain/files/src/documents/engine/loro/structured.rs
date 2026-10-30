//! Validating an import into a workbook or deck session. A workbook's
//! changed cells (by key, against the model before the import) are checked
//! with [`sheet::validate_sheet_changes`] against the sheets the workbook
//! then holds; a deck is checked whole with [`deck::validate_deck_changes`].
//! A rich document passes untouched: the collab layer validates it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::LoroDoc;

use super::sheet::{Sheet, SheetChange, SheetModel, validate_sheet_changes};
use super::{deck, sheet};
use crate::documents::engine::{EngineError, ModelView};

pub(super) fn check(
    doc: &LoroDoc,
    before: &ModelView,
    after: &ModelView,
) -> Result<(), EngineError> {
    match after {
        ModelView::Sheet(after) => {
            let empty = SheetModel::default();
            let before = match before {
                ModelView::Sheet(before) => before,
                _ => &empty,
            };
            validate_sheet_changes(&changes(before, after), &after.names())
                .map_err(|error| EngineError::Model(Box::new(error)))
        },
        ModelView::Deck(_) => deck::validate_deck_changes(doc)
            .map(|_| ())
            .map_err(|error| EngineError::Model(Box::new(error))),
        ModelView::RichText(_) | ModelView::Body(_) => Ok(()),
    }
}

fn changes(before: &SheetModel, after: &SheetModel) -> Vec<SheetChange> {
    after
        .sheets
        .iter()
        .flat_map(|sheet| {
            let prior = before.sheets.iter().find(|held| held.id == sheet.id);
            changed_cells(prior, sheet)
        })
        .collect()
}

fn changed_cells(prior: Option<&Sheet>, sheet: &Sheet) -> Vec<SheetChange> {
    sheet
        .cells
        .iter()
        .filter(|(key, cell)| prior.and_then(|held| held.cells.get(*key)) != Some(*cell))
        .map(|(key, cell)| {
            let (col, row) = position(sheet, key);
            SheetChange {
                sheet: sheet.name.clone(),
                col,
                row,
                v: cell.v.clone(),
            }
        })
        .collect()
}

/// A cell key's 1-based `(col, row)`; 0 for an id the sheet does not hold,
/// which the bounds check refuses.
fn position(sheet: &Sheet, key: &str) -> (u32, u32) {
    let (row_id, col_id) = key.split_once(':').unwrap_or((key, ""));
    let at = |ids: &[String], id: &str| {
        ids.iter()
            .position(|held| held == id)
            .and_then(|index| u32::try_from(index + 1).ok())
            .unwrap_or(0)
    };
    (at(&sheet.cols, col_id), at(&sheet.rows, row_id))
}

#[must_use]
pub(super) fn is_structured(doc: &LoroDoc) -> bool {
    sheet::is_sheet_doc(doc) || !deck::slides(doc).is_empty()
}
