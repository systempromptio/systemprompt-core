//! Translates the agent edit ops a workbook accepts into cell-sized writes
//! on the session's Loro sheet.
//!
//! `set_cells` writes each value of its range, `replace_text` on a cell
//! node writes that cell, and `insert_after` on a row node inserts a row
//! id after it and writes the tab-separated text across the new row. Every
//! write is validated with `validate_sheet_changes` before the first one
//! lands, so an op batch either applies whole or not at all. The caller
//! commits the doc under the agent's peer with origin `"agent"`.
//!
//! `apply_sheet_ops` answers the `sheet[Name]/A1` id of every cell it
//! wrote, each once.
//!
//! The Loro engine's `update_from_ops` calls `apply_sheet_ops` on a fork
//! of a workbook session's document; `ops::structured` logs the update.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


#[cfg(feature = "collab-loro")]
pub(crate) use translate::{SheetOpError, apply_sheet_ops};

#[cfg(feature = "collab-loro")]
mod translate {
    use loro::LoroDoc;
    use systemprompt_models::documents::EditOp;
    use systemprompt_provider_contracts::documents::{CellValue, NodeTarget};

    use crate::documents::engine::loro::rich::WriteError;
    use crate::documents::engine::loro::sheet::a1::{A1Range, SheetTarget, a1, parse_node};
    use crate::documents::engine::loro::sheet::edit::{
        ensure_cell_key, insert_row_after, set_cell_input, sheet_map,
    };
    use crate::documents::engine::loro::sheet::{
        SheetChange, SheetValidationError, read, validate_sheet_changes,
    };

    #[derive(Debug, thiserror::Error)]
    pub(crate) enum SheetOpError {
        #[error("operation {op} is not supported on a sheet")]
        Unsupported { op: &'static str },
        #[error("node {node} is not a sheet {expected} node")]
        BadNode {
            node: String,
            expected: &'static str,
        },
        #[error("range `{range}` is not an A1 range")]
        BadRange { range: String },
        #[error("set_cells values do not fit range {range}")]
        Shape { range: String },
        #[error(transparent)]
        Invalid(#[from] SheetValidationError),
        #[error("the sheet could not be read")]
        Read(#[from] crate::documents::engine::loro::sheet::SheetReadError),
        #[error("the sheet write failed: {0}")]
        Write(WriteError),
    }

    enum Write {
        Cell(SheetChange),
        Row {
            sheet: String,
            after: u32,
            text: String,
        },
    }

    pub(crate) fn apply_sheet_ops(
        doc: &LoroDoc,
        ops: &[EditOp],
    ) -> Result<Vec<(String, String)>, SheetOpError> {
        let writes = ops
            .iter()
            .map(plan)
            .collect::<Result<Vec<Vec<_>>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let changes: Vec<SheetChange> = writes
            .iter()
            .flat_map(|write| match write {
                Write::Cell(change) => vec![change.clone()],
                Write::Row { sheet, after, text } => row_changes(sheet, after + 1, text),
            })
            .collect();
        validate_sheet_changes(&changes, &read(doc)?.names())?;
        for write in &writes {
            match write {
                Write::Cell(change) => write_change(doc, change)?,
                Write::Row { sheet, after, text } => {
                    let map = sheet_map(doc, sheet).ok_or_else(|| unknown(sheet))?;
                    insert_row_after(&map, *after).map_err(SheetOpError::Write)?;
                    for change in row_changes(sheet, after + 1, text) {
                        write_change(doc, &change)?;
                    }
                },
            }
        }
        let mut touched: Vec<(String, String)> = Vec::new();
        for change in &changes {
            let id = format!(
                "sheet[{}]/{}",
                change.sheet.replace(']', "\\]"),
                a1(change.col, change.row)
            );
            touched.retain(|(held, _)| *held != id);
            touched.push((id, change.v.clone()));
        }
        Ok(touched)
    }

    fn unknown(sheet: &str) -> SheetOpError {
        SheetOpError::Invalid(SheetValidationError::UnknownSheet {
            sheet: sheet.to_owned(),
        })
    }

    fn write_change(doc: &LoroDoc, change: &SheetChange) -> Result<(), SheetOpError> {
        let map = sheet_map(doc, &change.sheet).ok_or_else(|| unknown(&change.sheet))?;
        let key = ensure_cell_key(&map, change.col, change.row).map_err(SheetOpError::Write)?;
        set_cell_input(&map, &key, &change.v).map_err(SheetOpError::Write)
    }

    fn row_changes(sheet: &str, row: u32, text: &str) -> Vec<SheetChange> {
        (1u32..)
            .zip(text.split('\t'))
            .filter(|(_, v)| !v.is_empty())
            .map(|(col, v)| SheetChange {
                sheet: sheet.to_owned(),
                col,
                row,
                v: v.to_owned(),
            })
            .collect()
    }

    fn plan(op: &EditOp) -> Result<Vec<Write>, SheetOpError> {
        match op {
            EditOp::SetCells {
                sheet,
                range,
                values,
                ..
            } => {
                let bad = || SheetOpError::BadRange {
                    range: range.clone(),
                };
                let area = A1Range::parse(range).ok_or_else(bad)?;
                let width = area.end.0 - area.start.0 + 1;
                let height = area.end.1 - area.start.1 + 1;
                let fits = values.len() == usize::try_from(height).unwrap_or(0)
                    && values
                        .iter()
                        .all(|row| row.len() == usize::try_from(width).unwrap_or(0));
                if !fits {
                    return Err(SheetOpError::Shape {
                        range: range.clone(),
                    });
                }
                Ok((area.start.1..)
                    .zip(values)
                    .flat_map(|(row, cells)| {
                        (area.start.0..).zip(cells).map(move |(col, value)| {
                            Write::Cell(SheetChange {
                                sheet: sheet.clone(),
                                col,
                                row,
                                v: input_of(value),
                            })
                        })
                    })
                    .collect())
            },
            EditOp::ReplaceText { node, text, .. } => match target(node) {
                Some((sheet, SheetTarget::Cell(col, row))) => Ok(vec![Write::Cell(SheetChange {
                    sheet,
                    col,
                    row,
                    v: text.clone(),
                })]),
                _ => Err(bad_node(node, "cell")),
            },
            EditOp::InsertAfter { node, text, .. } => match target(node) {
                Some((sheet, SheetTarget::Row(after))) => Ok(vec![Write::Row {
                    sheet,
                    after,
                    text: text.clone(),
                }]),
                Some((sheet, SheetTarget::Sheet)) => Ok(vec![Write::Row {
                    sheet,
                    after: 0,
                    text: text.clone(),
                }]),
                _ => Err(bad_node(node, "row")),
            },
            other => Err(SheetOpError::Unsupported { op: other.name() }),
        }
    }

    fn target(node: &NodeTarget) -> Option<(String, SheetTarget)> {
        match node {
            NodeTarget::Id(id) => parse_node(id.as_str()),
            NodeTarget::Find(_) => None,
        }
    }

    fn bad_node(node: &NodeTarget, expected: &'static str) -> SheetOpError {
        let node = match node {
            NodeTarget::Id(id) => id.as_str().to_owned(),
            NodeTarget::Find(quote) => quote.find.clone(),
        };
        SheetOpError::BadNode { node, expected }
    }

    fn input_of(value: &CellValue) -> String {
        match value {
            CellValue::Bool(true) => "TRUE".to_owned(),
            CellValue::Bool(false) => "FALSE".to_owned(),
            CellValue::Number(number) => number.to_string(),
            CellValue::Text(text) => text.clone(),
            CellValue::Empty => String::new(),
        }
    }
}
