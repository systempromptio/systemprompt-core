//! Applies edit operations to a workbook in memory; the caller serialises
//! the workbook only after every op succeeded, so a batch lands all or none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CellValue, CodecError, DocumentFormat, EditOp, hash8,
};
use umya_spreadsheet::{Workbook, Worksheet};

use super::workbook::{
    CellRange, Target, cell_display, load, node_id, parse_node, save, sheet_error, sheet_id,
    sheet_row_text,
};

pub(super) fn apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let mut workbook = load(bytes)?;
    for op in ops {
        apply_one(&mut workbook, op)?;
    }
    save(&workbook)
}

fn apply_one(workbook: &mut Workbook, op: &EditOp) -> Result<(), CodecError> {
    match op {
        EditOp::SetCells {
            sheet,
            range,
            values,
            expected_hash,
        } => set_cells(workbook, sheet, range, values, expected_hash.as_deref()),
        EditOp::ReplaceText {
            node,
            expected_hash,
            text,
        } => replace_text(workbook, node, expected_hash, text),
        EditOp::Delete {
            node,
            expected_hash,
        } => delete(workbook, node, expected_hash),
        EditOp::StrReplace { old_str, new_str } => str_replace(workbook, old_str, new_str),
        EditOp::InsertAfter { node, text, .. } => insert_after(workbook, node, text),
        EditOp::InsertSlide { .. } | EditOp::SetHtml { .. } | EditOp::SetText { .. } => {
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

fn resolve<'a>(
    workbook: &'a mut Workbook,
    node: &DocumentNodeId,
) -> Result<(&'a mut Worksheet, Target), CodecError> {
    let (name, target) =
        parse_node(node).ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
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
    sheet_name: &str,
    range: &str,
    values: &[Vec<CellValue>],
    expected_hash: Option<&str>,
) -> Result<(), CodecError> {
    let sheet = sheet_mut(workbook, sheet_name)?;
    let parsed = CellRange::parse(range).ok_or(CodecError::Malformed {
        detail: "set_cells range is not an A1 reference",
    })?;
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

fn write_cell(sheet: &mut Worksheet, col: u32, row: u32, value: &CellValue) {
    if matches!(value, CellValue::Empty) {
        if sheet.cell((col, row)).is_some() {
            sheet.cell_mut((col, row)).set_blank();
        }
        return;
    }
    let cell = sheet.cell_mut((col, row));
    match value {
        CellValue::Bool(flag) => {
            cell.set_value_bool(*flag);
        },
        CellValue::Number(number) => {
            cell.set_value_number(*number);
        },
        CellValue::Text(text) => match text.strip_prefix('=') {
            Some(formula) => {
                cell.set_blank();
                cell.set_formula(formula);
            },
            None => {
                cell.set_value_string(text.as_str());
            },
        },
        CellValue::Empty => {},
    }
}

fn auto_parse(text: &str) -> CellValue {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return CellValue::Empty;
    }
    if !trimmed.starts_with('=') {
        if let Ok(number) = trimmed.parse::<f64>() {
            return CellValue::Number(number);
        }
        if trimmed.eq_ignore_ascii_case("true") {
            return CellValue::Bool(true);
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return CellValue::Bool(false);
        }
    }
    CellValue::Text(text.to_owned())
}

fn write_row(sheet: &mut Worksheet, row: u32, text: &str) {
    for (col, value) in (1u32..).zip(text.split('\t')) {
        write_cell(sheet, col, row, &auto_parse(value));
    }
}

fn replace_text(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    expected: &str,
    text: &str,
) -> Result<(), CodecError> {
    let (sheet, target) = resolve(workbook, node)?;
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

fn clear_row(sheet: &mut Worksheet, row: u32) {
    let columns: Vec<u32> = sheet
        .collection_by_row(row)
        .into_iter()
        .map(|cell| cell.coordinate().col_num())
        .collect();
    for col in columns {
        sheet.cell_mut((col, row)).set_blank();
    }
}

fn delete(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    expected: &str,
) -> Result<(), CodecError> {
    let (sheet, target) = resolve(workbook, node)?;
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

fn insert_after(
    workbook: &mut Workbook,
    node: &DocumentNodeId,
    text: &str,
) -> Result<(), CodecError> {
    let (sheet, target) = resolve(workbook, node)?;
    let row = match target {
        Target::Row(row) | Target::Cell(_, row) => {
            sheet.insert_new_row(row + 1, 1);
            row + 1
        },
        Target::Sheet => sheet.highest_row() + 1,
    };
    write_row(sheet, row, text);
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
