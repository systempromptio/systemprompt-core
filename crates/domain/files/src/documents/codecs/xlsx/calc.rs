//! Moves a workbook between `.xlsx` bytes and the Loro [`SheetModel`]
//! through `IronCalc`, alongside the `umya-spreadsheet` edit path.
//!
//! [`sheet_model_from_xlsx`] reads every sheet's cell inputs (a formula as
//! `=…`) and merges, minting fresh row and column ids. [`checkpoint_xlsx`]
//! builds an `IronCalc` model from the sheet model, recalculates it and
//! writes it out, so the checkpoint's cached values are the ones a reader
//! without a calc engine sees. Bytes are made importable first
//! ([`ironcalc_input`](super::ironcalc_input)), since IronCalc panics on a
//! workbook missing a styles section. A cell's `style` is `IronCalc`'s own [`Style`]
//! as JSON (the shape the browser model's `getCellStyle` returns) and its
//! `fmt` is the number-format code; both are read on import and written back
//! at checkpoint, `fmt` overriding the style's `num_fmt`. Locale, timezone and
//! language match the browser's `IronCalc` model (`workbook`, `en`, `UTC`,
//! `en`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::Cursor;

use ironcalc::base::Model;
use ironcalc::base::types::{MergedCell, Style};
use ironcalc::export::save_xlsx_to_writer;
use ironcalc::import::load_from_xlsx_bytes;
use systemprompt_provider_contracts::documents::CodecError;

use super::ironcalc_input::{guarded, importable};
use crate::documents::engine::loro::sheet::a1::{A1Range, a1};
use crate::documents::engine::loro::sheet::{Sheet, SheetCell, SheetModel, seed_sheets};

const LOCALE: &str = "en";
const LANGUAGE: &str = "en";
const TIMEZONE: &str = "UTC";

fn xlsx_error(part: &str) -> impl FnOnce(ironcalc::error::XlsxError) -> CodecError + '_ {
    move |error| CodecError::Parse {
        part: part.to_owned(),
        source: Box::new(error),
    }
}

fn parse_error(part: &str) -> impl FnOnce(String) -> CodecError + '_ {
    move |detail| CodecError::Parse {
        part: part.to_owned(),
        source: detail.into(),
    }
}

pub fn sheet_model_from_xlsx(bytes: &[u8]) -> Result<SheetModel, CodecError> {
    let bytes = importable(bytes)?;
    let model = guarded("workbook", || {
        let workbook = load_from_xlsx_bytes(&bytes, "workbook", LOCALE, TIMEZONE)
            .map_err(xlsx_error("workbook"))?;
        Model::from_workbook(workbook, LANGUAGE).map_err(parse_error("workbook"))
    })?;
    let mut grids: BTreeMap<u32, Vec<(u32, u32, SheetCell)>> = BTreeMap::new();
    for index in model.get_all_cells() {
        let v = model
            .get_localized_cell_content(index.index, index.row, index.column)
            .map_err(parse_error("sheet"))?;
        let (Ok(col), Ok(row)) = (u32::try_from(index.column), u32::try_from(index.row)) else {
            continue;
        };
        let (fmt, style) = cell_style(&model, index.index, index.row, index.column);
        if !v.is_empty() || style.is_some() {
            grids
                .entry(index.index)
                .or_default()
                .push((col, row, SheetCell { v, fmt, style }));
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

fn cell_style(
    model: &Model<'_>,
    sheet: u32,
    row: i32,
    column: i32,
) -> (Option<String>, Option<String>) {
    let Ok(style) = model.get_style_for_cell(sheet, row, column) else {
        return (None, None);
    };
    if style == Style::default() {
        return (None, None);
    }
    let fmt = (style.num_fmt != Style::default().num_fmt).then(|| style.num_fmt.clone());
    (fmt, serde_json::to_string(&style).ok())
}

fn target_style(cell: &SheetCell) -> Option<Style> {
    let mut style = cell.style.as_deref().map_or_else(Style::default, |json| {
        serde_json::from_str(json)
            .inspect_err(|error| tracing::warn!(%error, "a cell style is not an IronCalc style"))
            .unwrap_or_default()
    });
    if let Some(fmt) = &cell.fmt {
        style.num_fmt.clone_from(fmt);
    }
    (style != Style::default()).then_some(style)
}

fn merge_a1(merge: &MergedCell) -> Option<String> {
    let col = u32::try_from(merge.column).ok()?;
    let row = u32::try_from(merge.row).ok()?;
    let last_col = u32::try_from(merge.column + merge.width - 1).ok()?;
    let last_row = u32::try_from(merge.row + merge.height - 1).ok()?;
    Some(format!("{}:{}", a1(col, row), a1(last_col, last_row)))
}

pub fn checkpoint_xlsx(sheets: &SheetModel) -> Result<Vec<u8>, CodecError> {
    let mut model = Model::new_empty("workbook", LOCALE, TIMEZONE, LANGUAGE)
        .map_err(parse_error("workbook"))?;
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
            if let Some(style) = target_style(cell) {
                model
                    .set_cell_style(index, row, column, &style)
                    .map_err(parse_error("style"))?;
            }
        }
        let merges = merged_cells(sheet);
        if let Some(worksheet) = model.workbook.worksheets.get_mut(index as usize) {
            worksheet.merged_cells = merges;
        }
    }
    model.evaluate();
    let cursor =
        save_xlsx_to_writer(&model, Cursor::new(Vec::new())).map_err(xlsx_error("workbook"))?;
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

pub fn seed_doc_from_xlsx(doc: &loro::LoroDoc, bytes: &[u8]) -> Result<bool, CodecError> {
    let model = sheet_model_from_xlsx(bytes)?;
    seed_sheets(doc, &model).map_err(|source| CodecError::Parse {
        part: "sheets".to_owned(),
        source,
    })
}
