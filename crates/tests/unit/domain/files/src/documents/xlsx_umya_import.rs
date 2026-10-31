use systemprompt_files::documents::codecs::xlsx::sheet_model_from_xlsx;
use systemprompt_files::documents::engine::loro::sheet::{Sheet, SheetCell};
use umya_spreadsheet::{HorizontalAlignmentValues, PatternValues};

fn write(book: &umya_spreadsheet::Workbook) -> Vec<u8> {
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut out).expect("write");
    out
}

fn at(sheet: &Sheet, col: u32, row: u32) -> Option<&SheetCell> {
    sheet
        .grid()
        .into_iter()
        .find(|(c, r, _)| *c == col && *r == row)
        .map(|(_, _, cell)| cell)
}

fn style(cell: &SheetCell) -> serde_json::Value {
    serde_json::from_str(cell.style.as_deref().expect("style")).expect("json")
}

fn styled_book() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.set_name("Styled");
    sheet.cell_mut("A1").set_value_string("fonts");
    {
        let font = sheet.cell_mut("A1").style_mut().font_mut();
        font.set_italic(true);
        font.set_underline("single");
        font.set_strikethrough(true);
        font.set_name("Georgia");
        font.set_size(14.4);
        font.color_mut().set_argb_str("FF112233");
    }
    sheet.cell_mut("B1").set_value_string("filled");
    sheet
        .cell_mut("B1")
        .style_mut()
        .fill_mut()
        .pattern_fill_mut()
        .set_pattern_type(PatternValues::Solid)
        .foreground_color_mut()
        .set_argb_str("FFAABBCC");
    sheet.cell_mut("C1").set_value_string("hatched");
    sheet
        .cell_mut("C1")
        .style_mut()
        .fill_mut()
        .pattern_fill_mut()
        .set_pattern_type(PatternValues::DarkGray)
        .foreground_color_mut()
        .set_argb_str("FF000000");
    for (coordinate, align) in [
        ("A2", HorizontalAlignmentValues::Right),
        ("B2", HorizontalAlignmentValues::Center),
        ("C2", HorizontalAlignmentValues::Justify),
        ("D2", HorizontalAlignmentValues::General),
    ] {
        sheet.cell_mut(coordinate).set_value_string("x");
        sheet
            .cell_mut(coordinate)
            .style_mut()
            .alignment_mut()
            .set_horizontal(align);
    }
    sheet.cell_mut("A3").set_value_number(0.5);
    sheet
        .cell_mut("A3")
        .style_mut()
        .number_format_mut()
        .set_format_code("0.0%");
    sheet.cell_mut("B3").set_value_number(7);
    sheet
        .cell_mut("B3")
        .style_mut()
        .number_format_mut()
        .set_format_code("General");
    sheet.cell_mut("A4").set_value_string("42");
    sheet.cell_mut("B4").set_value_string("=not a formula");
    sheet.cell_mut("C4").set_value_string("words");
    sheet.cell_mut("D4").set_formula("A3*2");
    sheet.add_merge_cells("A6:C7");
    sheet.add_merge_cells("not-a-range");
    let second = book.new_sheet("Second").expect("second");
    second.cell_mut("B2").set_value_number(3);
    write(&book)
}

#[test]
fn fonts_fills_alignment_and_number_formats_import_as_cell_styles() {
    let model = sheet_model_from_xlsx(&styled_book()).expect("import");
    assert_eq!(model.names(), vec!["Styled", "Second"]);
    let sheet = model.sheet("Styled").expect("sheet");

    let font = style(at(sheet, 1, 1).expect("a1"));
    assert_eq!(font["font"]["i"], serde_json::json!(true));
    assert_eq!(font["font"]["u"], serde_json::json!(true));
    assert_eq!(font["font"]["strike"], serde_json::json!(true));
    assert_eq!(font["font"]["name"], serde_json::json!("Georgia"));
    assert_eq!(font["font"]["sz"], serde_json::json!(14));
    assert!(font.to_string().contains("#112233"));

    let filled = style(at(sheet, 2, 1).expect("b1"));
    assert!(filled["fill"].to_string().contains("#AABBCC"));
    let hatched = at(sheet, 3, 1).expect("c1");
    assert!(
        hatched
            .style
            .as_deref()
            .is_none_or(|raw| !raw.contains("#000000"))
    );

    assert!(
        style(at(sheet, 1, 2).expect("a2"))
            .to_string()
            .contains("right")
    );
    assert!(
        style(at(sheet, 2, 2).expect("b2"))
            .to_string()
            .contains("center")
    );
    assert!(
        style(at(sheet, 3, 2).expect("c2"))
            .to_string()
            .contains("justify")
    );
    let general = at(sheet, 4, 2).expect("d2");
    assert!(
        general
            .style
            .as_deref()
            .is_none_or(|raw| !raw.contains("alignment\":{")),
        "{:?}",
        general.style
    );

    assert_eq!(at(sheet, 1, 3).expect("a3").fmt.as_deref(), Some("0.0%"));
    assert!(at(sheet, 2, 3).expect("b3").fmt.is_none());
}

#[test]
fn text_that_would_read_as_a_number_or_formula_is_quoted_and_formulas_keep_their_sign() {
    let model = sheet_model_from_xlsx(&styled_book()).expect("import");
    let sheet = model.sheet("Styled").expect("sheet");
    assert_eq!(at(sheet, 1, 4).expect("a4").v, "'42");
    assert_eq!(at(sheet, 2, 4).expect("b4").v, "'=not a formula");
    assert_eq!(at(sheet, 3, 4).expect("c4").v, "words");
    assert_eq!(at(sheet, 4, 4).expect("d4").v, "=A3*2");
    let merges: Vec<String> = sheet
        .merges
        .iter()
        .filter_map(|merge| sheet.merge_a1(merge))
        .collect();
    assert_eq!(merges, vec!["A6:C7".to_owned()]);
    let second = model.sheet("Second").expect("second");
    assert_eq!(at(second, 2, 2).expect("b2").v, "3");
}

#[test]
fn bytes_that_are_not_a_workbook_are_refused() {
    assert!(sheet_model_from_xlsx(b"PK\x03\x04 broken").is_err());
}
