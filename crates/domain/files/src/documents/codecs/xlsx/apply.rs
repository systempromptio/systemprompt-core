//! Applies edit operations to a workbook in memory; the caller serialises
//! the workbook only after every op succeeded, so a batch lands all or none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CellValue, CodecError, ContentAs, DocumentFormat, EditOp, hash8, required_hash,
};
use umya_spreadsheet::{Workbook, Worksheet};

use super::cells::{auto_parse, clear_row, write_cell, write_row};
use super::insert::{insert_after, insert_markdown};
use super::rebase::RowShifts;
use super::workbook::{
    CellRange, Target, cell_display, load, node_id, parse_node, save, sheet_error, sheet_id,
    sheet_row_text,
};

pub(super) fn apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    ops.iter().try_for_each(EditOp::check_resolved)?;
    let mut workbook = load(bytes)?;
    let mut shifts = RowShifts::default();
    for op in ops {
        apply_one(&mut workbook, op, &mut shifts)?;
    }
    save(&workbook)
}

fn apply_one(
    workbook: &mut Workbook,
    op: &EditOp,
    shifts: &mut RowShifts,
) -> Result<(), CodecError> {
    match op {
        EditOp::SetCells {
            sheet,
            range,
            values,
            expected_hash,
        } => set_cells(
            workbook,
            (sheet, range),
            values,
            expected_hash.as_deref(),
            shifts,
        ),
        EditOp::ReplaceText {
            node,
            expected_hash,
            text,
            ..
        } => replace_text(
            workbook,
            node.resolved()?,
            required_hash(expected_hash.as_ref())?,
            text,
            shifts,
        ),
        EditOp::Delete {
            node,
            expected_hash,
        } => delete(
            workbook,
            node.resolved()?,
            required_hash(expected_hash.as_ref())?,
            shifts,
        ),
        EditOp::StrReplace { old_str, new_str } => str_replace(workbook, old_str, new_str),
        EditOp::InsertAfter {
            node,
            text,
            content_as: ContentAs::Text,
            ..
        } => insert_after(workbook, node.resolved()?, text, shifts),
        EditOp::InsertAfter { node, text, .. } => {
            insert_markdown(workbook, Some(node.resolved()?), text, shifts)
        },
        EditOp::AppendMarkdown { markdown } => insert_markdown(workbook, None, markdown, shifts),
        EditOp::InsertSlide { .. }
        | EditOp::SetHtml { .. }
        | EditOp::SetText { .. }
        | EditOp::Restore { .. }
        | EditOp::InsertFromRevision { .. } => {
            Err(CodecError::UnsupportedOp {
                op: op.name(),
                format: DocumentFormat::Xlsx,
            })
        },
    }
}

fn sheet_mut<'a>(workbook: &'a mut Workbook, name: &str) -> Result<&'a mut Worksheet, CodecError> {
    workbook
        .sheet_by_name_mut(name)
        .map_err(sheet_error(node_id(sheet_id(name))))
}

pub(super) fn resolve<'a>(
    workbook: &'a mut Workbook,
    node: &DocumentNodeId,
    shifts: &RowShifts,
) -> Result<(&'a mut Worksheet, Target), CodecError> {
    let (name, target) =
        parse_node(node).ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
    let target = match target {
        Target::Row(row) => Target::Row(shifts.row(&name, row)),
        Target::Cell(col, row) => Target::Cell(col, shifts.row(&name, row)),
        Target::Sheet => Target::Sheet,
    };
    let sheet = workbook
        .sheet_by_name_mut(&name)
        .map_err(sheet_error(node.clone()))?;
    Ok((sheet, target))
}

fn check(node: &DocumentNodeId, expected: &str, current_text: String) -> Result<(), CodecError> {
    let actual = hash8(&current_text);
    if actual == expected {
        return Ok(());
    }
    Err(CodecError::StaleAnchor {
        node: node.clone(),
        expected: expected.to_owned(),
        actual,
        current_text,
    })
}

fn set_cells(
    workbook: &mut Workbook,
    (sheet_name, range): (&str, &str),
    values: &[Vec<CellValue>],
    expected_hash: Option<&str>,
    shifts: &RowShifts,
) -> Result<(), CodecError> {
    let sheet = sheet_mut(workbook, sheet_name)?;
    let mut parsed = CellRange::parse(range).ok_or(CodecError::Malformed {
        detail: "set_cells range is not an A1 reference",
    })?;
    parsed.start.1 = shifts.row(sheet_name, parsed.start.1);
    parsed.end.1 = shifts.row(sheet_name, parsed.end.1);
    let fits = usize::try_from(parsed.height()).is_ok_and(|h| h == values.len())
        && values
            .iter()
            .all(|row| usize::try_from(parsed.width()).is_ok_and(|w| w == row.len()));
    if !fits {
        return Err(CodecError::Malformed {
            detail: "set_cells values do not match the shape of the range",
        });
    }
    if let Some(expected) = expected_hash {
        let node = node_id(format!("{}/{range}", sheet_id(sheet_name)));
        check(&node, expected, parsed.text(sheet))?;
    }
    for (row, row_values) in (parsed.start.1..).zip(values) {
        for (col, value) in (parsed.start.0..).zip(row_values) {
            write_cell(sheet, col, row, value);
        }
    }
    Ok(())
}

fn replace_text(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    expected: &str,
    text: &str,
    shifts: &RowShifts,
) -> Result<(), CodecError> {
    let (sheet, target) = resolve(workbook, node, shifts)?;
    match target {
        Target::Cell(col, row) => {
            check(node, expected, cell_display(sheet, col, row))?;
            write_cell(sheet, col, row, &auto_parse(text));
        },
        Target::Row(row) => {
            check(node, expected, sheet_row_text(sheet, row))?;
            clear_row(sheet, row);
            write_row(sheet, row, text);
        },
        Target::Sheet => {
            return Err(CodecError::UnsupportedOp {
                op: "replace_text",
                format: DocumentFormat::Xlsx,
            });
        },
    }
    Ok(())
}

fn delete(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    expected: &str,
    shifts: &RowShifts,
) -> Result<(), CodecError> {
    let (sheet, target) = resolve(workbook, node, shifts)?;
    match target {
        Target::Cell(col, row) => {
            check(node, expected, cell_display(sheet, col, row))?;
            write_cell(sheet, col, row, &CellValue::Empty);
        },
        Target::Row(row) => {
            check(node, expected, sheet_row_text(sheet, row))?;
            clear_row(sheet, row);
        },
        Target::Sheet => {
            return Err(CodecError::UnsupportedOp {
                op: "delete",
                format: DocumentFormat::Xlsx,
            });
        },
    }
    Ok(())
}

fn str_replace(workbook: &mut Workbook, old_str: &str, new_str: &str) -> Result<(), CodecError> {
    if old_str.is_empty() {
        return Err(CodecError::Malformed {
            detail: "str_replace old_str is empty",
        });
    }
    let mut matches = 0usize;
    let mut hit = None;
    for (index, sheet) in workbook.sheet_collection().iter().enumerate() {
        for cell in sheet.cells() {
            if cell.is_formula() || cell.data_type() != "s" {
                continue;
            }
            let count = cell.value().matches(old_str).count();
            if count > 0 {
                matches += count;
                let coordinate = cell.coordinate();
                hit = Some((index, coordinate.col_num(), coordinate.row_num()));
            }
        }
    }
    match (matches, hit) {
        (0, _) | (_, None) => Err(CodecError::NoMatch),
        (1, Some((index, col, row))) => {
            let sheet = &mut workbook.sheet_collection_mut()[index];
            let cell = sheet.cell_mut((col, row));
            let replaced = cell.value().replacen(old_str, new_str, 1);
            cell.set_value_string(replaced);
            Ok(())
        },
        (matches, Some(_)) => Err(CodecError::AmbiguousMatch { matches }),
    }
}
