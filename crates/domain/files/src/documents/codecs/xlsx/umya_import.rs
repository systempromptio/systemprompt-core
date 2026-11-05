//! Reads a workbook into an `IronCalc` [`Model`] through `umya-spreadsheet`.
//!
//! `IronCalc`'s own `.xlsx` importer indexes optional parts of the package
//! (a styles section, `sheetViews`, `cols`, `sheetPr`, …) without checking
//! that they exist, and the release profile aborts on panic, so a valid
//! workbook that leaves one out would take the process down. The package is
//! therefore parsed by `umya-spreadsheet`, which the edit path already
//! trusts, and the model is built through `IronCalc`'s API: each sheet by
//! name, each cell's input (a formula as `=…`, a text that would read as a
//! number or formula behind a `'`), its style and the merges.
//!
//! A cell's style carries its number format, bold, italic, underline,
//! strike, an RGB font colour, a font name and size other than the workbook
//! default (Calibri 11), a solid RGB fill and the horizontal alignment.
//! Theme colours and tints are resolved to RGB against the workbook's theme.
//! A font in plain black, the theme's usual text colour, stays the default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use ironcalc::base::Model;
use ironcalc::base::types::{Alignment, Color, HorizontalAlignment, MergedCell, Style};
use systemprompt_provider_contracts::documents::CodecError;
use umya_spreadsheet::drawing::Theme;
use umya_spreadsheet::{Cell, HorizontalAlignmentValues, PatternValues, Worksheet};

use super::workbook::load;
use crate::documents::engine::loro::sheet::a1::A1Range;

const DEFAULT_FONT: &str = "Calibri";
const DEFAULT_SIZE: f64 = 11.0;

fn parse_error(part: &str) -> impl FnOnce(String) -> CodecError + '_ {
    move |detail| CodecError::Parse {
        part: part.to_owned(),
        source: detail.into(),
    }
}

fn rgb(value: &str) -> Option<Color> {
    let rgb = match value.len() {
        8 if value != "00000000" => value.get(2..)?,
        6 => value,
        _ => return None,
    };
    Some(Color::Rgb(format!("#{}", rgb.to_ascii_uppercase())))
}

const fn horizontal(value: &HorizontalAlignmentValues) -> HorizontalAlignment {
    match value {
        HorizontalAlignmentValues::Center => HorizontalAlignment::Center,
        HorizontalAlignmentValues::CenterContinuous => HorizontalAlignment::CenterContinuous,
        HorizontalAlignmentValues::Distributed => HorizontalAlignment::Distributed,
        HorizontalAlignmentValues::Fill => HorizontalAlignment::Fill,
        HorizontalAlignmentValues::Justify => HorizontalAlignment::Justify,
        HorizontalAlignmentValues::Left => HorizontalAlignment::Left,
        HorizontalAlignmentValues::Right => HorizontalAlignment::Right,
        HorizontalAlignmentValues::General => HorizontalAlignment::General,
    }
}

fn style_of(cell: &Cell, theme: &Theme) -> Option<Style> {
    let source = cell.style();
    let mut style = Style::default();
    if let Some(format) = source.number_format() {
        let code = format.format_code();
        if !code.is_empty() && !code.eq_ignore_ascii_case("general") {
            code.clone_into(&mut style.num_fmt);
        }
    }
    if let Some(font) = source.font() {
        style.font.b = font.bold();
        style.font.i = font.italic();
        style.font.u = !matches!(font.underline(), "" | "none");
        style.font.strike = font.strikethrough();
        if let Some(color) = rgb(&font.color().argb_with_theme(theme))
            .filter(|color| !matches!(color, Color::Rgb(hex) if hex == "#000000"))
        {
            style.font.color = color;
        }
        if font.name() != DEFAULT_FONT || (font.size() - DEFAULT_SIZE).abs() > f64::EPSILON {
            font.name().clone_into(&mut style.font.name);
            style.font.sz = font.size().round() as i32;
        }
    }
    if let Some(pattern) = source.fill().and_then(|fill| fill.pattern_fill())
        && matches!(pattern.pattern_type(), PatternValues::Solid)
        && let Some(color) = pattern
            .foreground_color()
            .and_then(|color| rgb(&color.argb_with_theme(theme)))
    {
        style.fill.color = color;
    }
    if let Some(alignment) = source.alignment() {
        let align = horizontal(alignment.horizontal());
        if align != HorizontalAlignment::General {
            style.alignment = Some(Alignment {
                horizontal: align,
                ..Alignment::default()
            });
        }
    }
    (style != Style::default()).then_some(style)
}

fn input_of(cell: &Cell) -> String {
    if cell.is_formula() {
        return format!("={}", cell.formula());
    }
    let value = cell.value().into_owned();
    let reads_otherwise = value.starts_with('=') || value.trim().parse::<f64>().is_ok();
    if cell.data_type() == "s" && reads_otherwise {
        format!("'{value}")
    } else {
        value
    }
}

fn merges(sheet: &Worksheet) -> Vec<MergedCell> {
    sheet
        .merge_cells()
        .iter()
        .filter_map(|range| A1Range::parse(&range.range()))
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

pub(super) fn model_from_xlsx<'a>(
    bytes: &[u8],
    (locale, timezone, language): (&'a str, &'a str, &'a str),
) -> Result<Model<'a>, CodecError> {
    let workbook = load(bytes)?;
    let mut model = Model::new_empty("workbook", locale, timezone, language)
        .map_err(parse_error("workbook"))?;
    for (index, sheet) in (0u32..).zip(workbook.sheet_collection()) {
        if index == 0 {
            model
                .rename_sheet_by_index(0, sheet.name())
                .map_err(parse_error("sheet"))?;
        } else {
            model
                .add_sheet(sheet.name())
                .map_err(parse_error("sheet"))?;
        }
        for cell in sheet.cells() {
            let coordinate = cell.coordinate();
            let (Ok(row), Ok(column)) = (
                i32::try_from(coordinate.row_num()),
                i32::try_from(coordinate.col_num()),
            ) else {
                continue;
            };
            let input = input_of(cell);
            if !input.is_empty() {
                model
                    .set_user_input(index, row, column, input)
                    .map_err(parse_error("cell"))?;
            }
            if let Some(style) = style_of(cell, workbook.theme()) {
                model
                    .set_cell_style(index, row, column, &style)
                    .map_err(parse_error("style"))?;
            }
        }
        if let Some(worksheet) = model.workbook.worksheets.get_mut(index as usize) {
            worksheet.merged_cells = merges(sheet);
        }
    }
    Ok(model)
}
