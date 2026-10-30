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

/// Checks every change against `sheet_names`, the grid bounds, the input
/// length and the formula grammar. Formulas may reference any sheet in
/// `sheet_names`.
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
            formula::check(formula, change, sheet_names)
                .map_err(|message| SheetValidationError::Formula { cell, message })?;
        }
    }
    Ok(())
}

#[cfg(feature = "office")]
mod formula {
    use std::collections::HashMap;

    use ironcalc::base::expressions::parser::{Node, new_parser_english};
    use ironcalc::base::expressions::types::CellReferenceRC;

    use super::SheetChange;

    pub(super) fn check(
        formula: &str,
        change: &SheetChange,
        sheet_names: &[String],
    ) -> Result<(), String> {
        let mut parser = new_parser_english(sheet_names.to_vec(), Vec::new(), HashMap::new());
        let context = CellReferenceRC {
            sheet: change.sheet.clone(),
            column: i32::try_from(change.col).map_err(|e| e.to_string())?,
            row: i32::try_from(change.row).map_err(|e| e.to_string())?,
        };
        match parser.parse(formula, &context) {
            Node::ParseErrorKind { message, .. } => Err(message),
            _ => Ok(()),
        }
    }
}

#[cfg(not(feature = "office"))]
mod formula {
    use super::SheetChange;

    pub(super) const fn check(
        _formula: &str,
        _change: &SheetChange,
        _sheet_names: &[String],
    ) -> Result<(), String> {
        Ok(())
    }
}
