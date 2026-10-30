//! Per-cell validation of sheet edits, sized by the change and never by
//! the workbook.
//!
//! The apply path collects one [`SheetChange`] per cell an update or an
//! agent op writes and calls [`validate_sheet_changes`] before it commits.
//! Each change is checked for its address against the sheet limits, its
//! input length and, with the `office` feature, that a formula parses with
//! IronCalc's parser.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;
pub const MAX_INPUT_CHARS: usize = 32_767;
pub const MAX_CHANGES: usize = 100_000;

/// One cell write: 1-based `(col, row)` on the sheet named `sheet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetChange {
    pub sheet: String,
    pub col: u32,
    pub row: u32,
    pub v: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SheetValidationError {
    #[error("{count} cell changes exceed the {max} change bound")]
    TooManyChanges { count: usize, max: usize },
    #[error("sheet `{sheet}` does not exist")]
    UnknownSheet { sheet: String },
    #[error("cell ({col}, {row}) is outside the {MAX_COLS} x {MAX_ROWS} grid")]
    OutOfBounds { col: u32, row: u32 },
    #[error("cell {cell} holds {chars} characters, over the {MAX_INPUT_CHARS} bound")]
    TooLong { cell: String, chars: usize },
    #[error("cell {cell} has a formula that does not parse: {message}")]
    Formula { cell: String, message: String },
}

pub fn validate_sheet_changes(
    changes: &[SheetChange],
    sheet_names: &[String],
) -> Result<(), SheetValidationError> {
    if changes.len() > MAX_CHANGES {
        return Err(SheetValidationError::TooManyChanges {
            count: changes.len(),
            max: MAX_CHANGES,
        });
    }
    for change in changes {
        if !sheet_names.iter().any(|name| *name == change.sheet) {
            return Err(SheetValidationError::UnknownSheet {
                sheet: change.sheet.clone(),
            });
        }
        if change.col == 0 || change.row == 0 || change.col > MAX_COLS || change.row > MAX_ROWS {
            return Err(SheetValidationError::OutOfBounds {
                col: change.col,
                row: change.row,
            });
        }
        let chars = change.v.chars().count();
        let cell = format!("{}!{}", change.sheet, super::a1::a1(change.col, change.row));
        if chars > MAX_INPUT_CHARS {
            return Err(SheetValidationError::TooLong { cell, chars });
        }
        if let Some(formula) = change.v.strip_prefix('=') {
            formula::check(formula, change, sheet_names, &cell)?;
        }
    }
    Ok(())
}

#[cfg(feature = "office")]
mod formula {
    use std::collections::HashMap;

    use ironcalc::base::expressions::parser::{Node, new_parser_english};
    use ironcalc::base::expressions::types::CellReferenceRC;

    use super::{SheetChange, SheetValidationError};

    pub(super) fn check(
        formula: &str,
        change: &SheetChange,
        sheet_names: &[String],
        cell: &str,
    ) -> Result<(), SheetValidationError> {
        let out_of_bounds = |_| SheetValidationError::OutOfBounds {
            col: change.col,
            row: change.row,
        };
        let mut parser = new_parser_english(sheet_names.to_vec(), Vec::new(), HashMap::new());
        let context = CellReferenceRC {
            sheet: change.sheet.clone(),
            column: i32::try_from(change.col).map_err(out_of_bounds)?,
            row: i32::try_from(change.row).map_err(out_of_bounds)?,
        };
        match parser.parse(formula, &context) {
            Node::ParseErrorKind { message, .. } => Err(SheetValidationError::Formula {
                cell: cell.to_owned(),
                message,
            }),
            _ => Ok(()),
        }
    }
}

#[cfg(not(feature = "office"))]
mod formula {
    use super::{SheetChange, SheetValidationError};

    pub(super) const fn check(
        _formula: &str,
        _change: &SheetChange,
        _sheet_names: &[String],
        _cell: &str,
    ) -> Result<(), SheetValidationError> {
        Ok(())
    }
}
