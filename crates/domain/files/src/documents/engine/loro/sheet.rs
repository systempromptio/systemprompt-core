//! The workbook model held in Loro containers.
//!
//! The root [`loro::LoroMovableList`] named [`SHEETS_CONTAINER`] holds one
//! [`loro::LoroMap`] per worksheet: `id`, `name`, `rows` and `cols` (each a
//! movable list of opaque string ids), `cells` (a map keyed
//! `"<rowId>:<colId>"` whose values are maps of `v` — the input text, a
//! formula as `=…` — `fmt` and `style`, a JSON string) and `merges` (a list
//! of `"<r0>:<c0>|<r1>:<c1>"` id ranges). A cell's A1 address is derived
//! from the positions of its ids, so inserting a row moves every later
//! address without rewriting a single cell, and two replicas inserting rows
//! at once both keep their edits. No computed value is ever stored here:
//! values are recalculated by IronCalc on the client and at checkpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod a1;
pub mod edit;
pub mod model;
mod read;
pub mod validate;
mod write;

pub use model::{Sheet, SheetCell, SheetModel, cell_key, new_id};
pub use read::{SheetReadError, read};
pub use validate::{SheetChange, SheetValidationError, validate_sheet_changes};
pub use write::{seed_sheets, write_sheets};

pub const SHEETS_CONTAINER: &str = "sheets";

#[must_use]
pub fn is_sheet_doc(doc: &loro::LoroDoc) -> bool {
    !doc.get_movable_list(SHEETS_CONTAINER).is_empty()
}
