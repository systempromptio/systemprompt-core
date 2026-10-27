//! Row inserts: one row of tab-separated text, or Markdown blocks lowered
//! into rows.
//!
//! Rows are inserted below the anchor row or cell, or appended below
//! the last used row of the anchor sheet (the first sheet for
//! `append_markdown`). A table writes one row per table row, one cell per
//! column, with numbers and booleans typed as `set_cells` types them; every
//! other block writes a single-cell row holding its plain text, list items
//! keeping their marker. Anchors are rebased through [`RowShifts`], so a
//! second insert after the same row lands after the first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CellValue, CodecError, DocumentFormat};
use umya_spreadsheet::{Workbook, Worksheet};

use super::apply::resolve;
use super::cells::{auto_parse, write_cell, write_row};
use super::rebase::RowShifts;
use super::workbook::{Target, parse_node};
use crate::documents::codecs::markdown_lowering::{Block, lower, plain};

fn rows(markdown: &str) -> Vec<Vec<CellValue>> {
    lower(markdown)
        .iter()
        .flat_map(|block| match block {
            Block::Table(rows) => rows
                .iter()
                .map(|row| row.iter().map(|cell| auto_parse(&plain(cell))).collect())
                .collect(),
            other => vec![vec![CellValue::Text(other.plain())]],
        })
        .collect()
}

fn write_values(sheet: &mut Worksheet, row: u32, values: &[CellValue]) {
    for (col, value) in (1u32..).zip(values) {
        write_cell(sheet, col, row, value);
    }
}

fn insert_rows<F>(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    count: usize,
    shifts: &mut RowShifts,
    mut write: F,
) -> Result<(), CodecError>
where
    F: FnMut(&mut Worksheet, u32, usize),
{
    let (name, original) =
        parse_node(node).ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
    let (sheet, target) = resolve(workbook, node, shifts)?;
    let (Target::Row(_) | Target::Cell(..), Target::Row(anchor) | Target::Cell(_, anchor)) =
        (target, original)
    else {
        let first = sheet.highest_row() + 1;
        for (row, index) in (first..).zip(0..count) {
            write(sheet, row, index);
        }
        return Ok(());
    };
    for index in 0..count {
        let at = shifts.anchor(&name, anchor) + 1;
        sheet.insert_new_row(at, 1);
        write(sheet, at, index);
        shifts.inserted(&name, anchor, at);
    }
    Ok(())
}

pub(super) fn insert_markdown(
    workbook: &mut Workbook,
    node: Option<&DocumentNodeId>,
    markdown: &str,
    shifts: &mut RowShifts,
) -> Result<(), CodecError> {
    let rows = rows(markdown);
    if let Some(node) = node {
        return insert_rows(workbook, node, rows.len(), shifts, |sheet, row, index| {
            write_values(sheet, row, &rows[index]);
        });
    }
    let sheet = workbook
        .sheet_collection_mut()
        .first_mut()
        .ok_or(CodecError::UnsupportedOp {
            op: "append_markdown",
            format: DocumentFormat::Xlsx,
        })?;
    let first = sheet.highest_row() + 1;
    for (row, values) in (first..).zip(&rows) {
        write_values(sheet, row, values);
    }
    Ok(())
}

pub(super) fn insert_after(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    text: &str,
    shifts: &mut RowShifts,
) -> Result<(), CodecError> {
    insert_rows(workbook, node, 1, shifts, |sheet, row, _| {
        write_row(sheet, row, text);
    })
}
