//! Cell and row writes shared by the workbook edit operations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CellValue;
use umya_spreadsheet::Worksheet;

pub(super) fn write_cell(sheet: &mut Worksheet, col: u32, row: u32, value: &CellValue) {
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


pub(super) fn auto_parse(text: &str) -> CellValue {
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


pub(super) fn write_row(sheet: &mut Worksheet, row: u32, text: &str) {
    for (col, value) in (1u32..).zip(text.split('\t')) {
        write_cell(sheet, col, row, &auto_parse(value));
    }
}


pub(super) fn clear_row(sheet: &mut Worksheet, row: u32) {
    let columns: Vec<u32> = sheet
        .collection_by_row(row)
        .into_iter()
        .map(|cell| cell.coordinate().col_num())
        .collect();
    for col in columns {
        sheet.cell_mut((col, row)).set_blank();
    }
}
