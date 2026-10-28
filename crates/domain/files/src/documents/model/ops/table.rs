//! The table ops on a model `Table` block.
//!
//! Rows and columns count from zero and `at` defaults to the end. A new row
//! is as wide as the widest row; a new column adds one cell to every row,
//! padding short rows first. Each new cell is one paragraph holding its text,
//! under a fresh block id. Deleting the last row or column leaves no table,
//! so the block goes. A table the model keeps as native markup (`Raw`) can
//! only be deleted whole.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::{CodecError, TableDelete, TableInsert};

use super::FreshIds;
use crate::documents::model::{Block, BlockKind, RichText};

const fn malformed(detail: &'static str) -> CodecError {
    CodecError::Malformed { detail }
}

pub(super) fn cell(fresh: &mut FreshIds, text: Option<&String>) -> RichText {
    RichText {
        blocks: vec![Block::new(
            fresh.next_id(),
            BlockKind::Paragraph,
            text.cloned().unwrap_or_default(),
        )],
    }
}

fn rows_mut(block: &mut Block) -> Result<&mut Vec<Vec<RichText>>, CodecError> {
    match &mut block.kind {
        BlockKind::Table { rows } => Ok(rows),
        BlockKind::Raw { .. } => Err(malformed(
            "this table is native markup the model keeps verbatim; it can only be deleted whole",
        )),
        _ => Err(malformed("the node is not a table")),
    }
}

fn width(rows: &[Vec<RichText>]) -> usize {
    rows.iter().map(Vec::len).max().unwrap_or(0)
}

pub(super) fn insert_row(
    block: &mut Block,
    fresh: &mut FreshIds,
    op: &TableInsert,
) -> Result<(), CodecError> {
    let rows = rows_mut(block)?;
    let columns = width(rows).max(op.cells.len()).max(1);
    if !rows.is_empty() && op.cells.len() > width(rows) {
        return Err(malformed(
            "the row has more cells than the table has columns",
        ));
    }
    let at = op.at.unwrap_or(rows.len());
    if at > rows.len() {
        return Err(malformed("the row index is past the end of the table"));
    }
    let row = (0..columns).map(|c| cell(fresh, op.cells.get(c))).collect();
    rows.insert(at, row);
    Ok(())
}

pub(super) fn insert_col(
    block: &mut Block,
    fresh: &mut FreshIds,
    op: &TableInsert,
) -> Result<(), CodecError> {
    let rows = rows_mut(block)?;
    let columns = width(rows);
    let at = op.at.unwrap_or(columns);
    if at > columns {
        return Err(malformed("the column index is past the end of the table"));
    }
    if op.cells.len() > rows.len() {
        return Err(malformed(
            "the column has more cells than the table has rows",
        ));
    }
    for (index, row) in rows.iter_mut().enumerate() {
        while row.len() < columns {
            row.push(cell(fresh, None));
        }
        row.insert(at, cell(fresh, op.cells.get(index)));
    }
    Ok(())
}

pub(super) enum Remaining {
    Table,
    Nothing,
}

pub(super) fn delete(block: &mut Block, op: &TableDelete) -> Result<Remaining, CodecError> {
    match (op.row, op.col) {
        (None, None) => Ok(Remaining::Nothing),
        (Some(_), Some(_)) => Err(malformed("table_delete names a row or a column, not both")),
        (Some(row), None) => {
            let rows = rows_mut(block)?;
            if row >= rows.len() {
                return Err(malformed("the row index is past the end of the table"));
            }
            rows.remove(row);
            Ok(left(rows))
        },
        (None, Some(col)) => {
            let rows = rows_mut(block)?;
            if col >= width(rows) {
                return Err(malformed("the column index is past the end of the table"));
            }
            for row in rows.iter_mut().filter(|row| col < row.len()) {
                row.remove(col);
            }
            Ok(left(rows))
        },
    }
}

fn left(rows: &[Vec<RichText>]) -> Remaining {
    if width(rows) == 0 {
        Remaining::Nothing
    } else {
        Remaining::Table
    }
}
