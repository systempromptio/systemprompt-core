//! Moves a workbook between `.xlsx` bytes and the Loro [`SheetModel`]
//! through IronCalc, alongside the `umya-spreadsheet` edit path.
//!
//! [`sheet_model_from_xlsx`] reads every sheet's cell inputs (a formula as
//! `=…`) and merges, minting fresh row and column ids. [`checkpoint_xlsx`]
//! builds an IronCalc model from the sheet model, recalculates it and
//! writes it out, so the checkpoint's cached values are the ones a reader
//! without a calc engine sees. Number formats and styles held in the sheet
//! model are not yet written back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::Cursor;

use ironcalc::base::Model;
use ironcalc::base::types::MergedCell;
use ironcalc::export::save_xlsx_to_writer;
use ironcalc::import::load_from_xlsx_bytes;
use systemprompt_provider_contracts::documents::CodecError;

use crate::documents::engine::loro::sheet::a1::{A1Range, a1};
use crate::documents::engine::loro::sheet::{Sheet, SheetCell, SheetModel};

const LOCALE: &str = "en";
const LANGUAGE: &str = "en";
const TIMEZONE: &str = "UTC";

fn parse_error(part: &str) -> impl FnOnce(String) -> CodecError + '_ {
    move |detail| CodecError::Parse {
        part: part.to_owned(),
        source: detail.into(),
    }
}

pub fn sheet_model_from_xlsx(bytes: &[u8]) -> Result<SheetModel, CodecError> {
    let workbook = load_from_xlsx_bytes(bytes, "workbook", LOCALE, TIMEZONE)
        .map_err(|error| parse_error("workbook")(error.to_string()))?;
    let model = Model::from_workbook(workbook, LANGUAGE).map_err(parse_error("workbook"))?;
    let mut grids: BTreeMap<u32, Vec<(u32, u32, SheetCell)>> = BTreeMap::new();
    for index in model.get_all_cells() {
        let v = model
            .get_localized_cell_content(index.index, index.row, index.column)
            .map_err(parse_error("sheet"))?;
        let (Ok(col), Ok(row)) = (u32::try_from(index.column), u32::try_from(index.row)) else {
            continue;
        };
        if !v.is_empty() {
            grids.entry(index.index).or_default().push((
                col,
                row,
                SheetCell {
                    v,
                    ..SheetCell::default()
                },
            ));
        }
    }
    let sheets = (0u32..)
        .zip(&model.workbook.worksheets)
        .map(|(index, worksheet)| {
            let merges: Vec<String> = worksheet.merged_cells.iter().filter_map(merge_a1).collect();
            Sheet::from_grid(
                &worksheet.name,
                grids.remove(&index).unwrap_or_default(),
                &merges,
            )
        })
        .collect();
    Ok(SheetModel { sheets })
}

fn merge_a1(merge: &MergedCell) -> Option<String> {
    let col = u32::try_from(merge.column).ok()?;
    let row = u32::try_from(merge.row).ok()?;
    let last_col = u32::try_from(merge.column + merge.width - 1).ok()?;
    let last_row = u32::try_from(merge.row + merge.height - 1).ok()?;
    Some(format!("{}:{}", a1(col, row), a1(last_col, last_row)))
}

/// The `.xlsx` bytes of `sheets`, recalculated so every formula cell
/// carries its cached value.
pub fn checkpoint_xlsx(sheets: &SheetModel) -> Result<Vec<u8>, CodecError> {
    let mut model =
        Model::new_empty("workbook", LOCALE, TIMEZONE, LANGUAGE).map_err(parse_error("workbook"))?;
    for (index, sheet) in (0u32..).zip(&sheets.sheets) {
        if index == 0 {
            model
                .rename_sheet_by_index(0, &sheet.name)
                .map_err(parse_error("sheet"))?;
        } else {
            model.add_sheet(&sheet.name).map_err(parse_error("sheet"))?;
        }
        for (col, row, cell) in sheet.grid() {
            let (Ok(column), Ok(row)) = (i32::try_from(col), i32::try_from(row)) else {
                continue;
            };
            model
                .set_user_input(index, row, column, cell.v.clone())
                .map_err(parse_error("cell"))?;
        }
        let merges = merged_cells(sheet);
        if let Some(worksheet) = model.workbook.worksheets.get_mut(index as usize) {
            worksheet.merged_cells = merges;
        }
    }
    model.evaluate();
    let cursor = save_xlsx_to_writer(&model, Cursor::new(Vec::new()))
        .map_err(|error| parse_error("workbook")(error.to_string()))?;
    Ok(cursor.into_inner())
}

fn merged_cells(sheet: &Sheet) -> Vec<MergedCell> {
    sheet
        .merges
        .iter()
        .filter_map(|merge| A1Range::parse(&sheet.merge_a1(merge)?))
        .filter_map(|range| {
            Some(MergedCell {
                row: i32::try_from(range.start.1).ok()?,
                column: i32::try_from(range.start.0).ok()?,
                width: i32::try_from(range.end.0 - range.start.0 + 1).ok()?,
                height: i32::try_from(range.end.1 - range.start.1 + 1).ok()?,
            })
        })
        .collect()
}
