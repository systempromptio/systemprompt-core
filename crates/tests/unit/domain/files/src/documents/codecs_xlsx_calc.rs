use std::io::Cursor;

use systemprompt_files::documents::codecs::xlsx::{
    checkpoint_xlsx, seed_doc_from_xlsx, sheet_model_from_xlsx,
};
use systemprompt_files::documents::engine::loro::sheet::{
    Sheet, SheetCell, SheetModel, read as read_sheets,
};
use systemprompt_provider_contracts::documents::CodecError;

fn cell(v: &str) -> SheetCell {
    SheetCell {
        v: v.to_owned(),
        fmt: None,
        style: None,
    }
}

fn model() -> SheetModel {
    let budget = Sheet::from_grid(
        "Budget",
        vec![
            (1, 1, cell("Item")),
            (2, 1, cell("Cost")),
            (1, 2, cell("Rent")),
            (2, 2, cell("1200")),
            (1, 3, cell("Food")),
            (2, 3, cell("300")),
            (2, 4, cell("=SUM(B2:B3)")),
            (3, 4, cell("=B4*2")),
        ],
        &["A5:C5".to_owned()],
    );
    let summary = Sheet::from_grid(
        "Summary",
        vec![
            (1, 1, cell("=Budget!B4/3")),
            (2, 1, cell("=UPPER(Budget!A2)")),
        ],
        &[],
    );
    SheetModel {
        sheets: vec![budget, summary],
    }
}

fn workbook(bytes: &[u8]) -> umya_spreadsheet::Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).expect("read")
}

fn at(sheet: &Sheet, col: u32, row: u32) -> Option<&SheetCell> {
    sheet
        .grid()
        .into_iter()
        .find(|(c, r, _)| *c == col && *r == row)
        .map(|(_, _, cell)| cell)
}

#[test]
fn a_checkpoint_carries_recalculated_values_for_every_formula() {
    let bytes = checkpoint_xlsx(&model()).expect("checkpoint");
    let book = workbook(&bytes);
    let budget = book.sheet_by_name("Budget").expect("budget");
    assert_eq!(budget.value("B4"), "1500");
    assert_eq!(budget.value("C4"), "3000");
    assert_eq!(budget.cell("B4").expect("b4").formula(), "SUM(B2:B3)");
    let summary = book.sheet_by_name("Summary").expect("summary");
    assert_eq!(summary.value("A1"), "500");
    assert_eq!(summary.value("B1"), "RENT");
}

#[test]
fn sheets_merges_and_formulas_survive_model_to_bytes_to_model() {
    let bytes = checkpoint_xlsx(&model()).expect("checkpoint");
    let back = sheet_model_from_xlsx(&bytes).expect("read");
    assert_eq!(back.names(), vec!["Budget", "Summary"]);
    let budget = back.sheet("Budget").expect("budget");
    assert_eq!(at(budget, 1, 2).map(|c| c.v.as_str()), Some("Rent"));
    assert_eq!(at(budget, 2, 2).map(|c| c.v.as_str()), Some("1200"));
    assert_eq!(at(budget, 2, 4).map(|c| c.v.as_str()), Some("=SUM(B2:B3)"));
    let merges: Vec<String> = budget
        .merges
        .iter()
        .filter_map(|m| budget.merge_a1(m))
        .collect();
    assert_eq!(merges, vec!["A5:C5"]);
    let summary = back.sheet("Summary").expect("summary");
    assert_eq!(
        at(summary, 1, 1).map(|c| c.v.as_str()),
        Some("=Budget!B4/3")
    );
    let again = checkpoint_xlsx(&back).expect("second checkpoint");
    assert_eq!(
        workbook(&again)
            .sheet_by_name("Summary")
            .expect("summary")
            .value("A1"),
        "500"
    );
}

#[test]
fn a_number_format_overrides_the_style_and_reads_back_as_fmt() {
    let style = r##"{"read_only":false,"quote_prefix":false,"fill":{"pattern_type":"none"},"font":{"strike":false,"u":false,"b":true,"i":false,"sz":13,"color":"#000000","name":"Calibri","family":2,"scheme":"minor"},"border":{"diagonal_up":false,"diagonal_down":false},"num_fmt":"general"}"##;
    let mut money = cell("12.5");
    money.fmt = Some("0.00%".to_owned());
    let mut bold = cell("Total");
    bold.style = Some(style.to_owned());
    let sheets = SheetModel {
        sheets: vec![Sheet::from_grid(
            "Sheet1",
            vec![(1, 1, money), (2, 1, bold)],
            &[],
        )],
    };
    let back = sheet_model_from_xlsx(&checkpoint_xlsx(&sheets).expect("checkpoint")).expect("read");
    let sheet = &back.sheets[0];
    assert_eq!(sheet.name, "Sheet1");
    let a1 = at(sheet, 1, 1).expect("a1");
    assert_eq!(a1.fmt.as_deref(), Some("0.00%"));
    let b1 = at(sheet, 2, 1).expect("b1");
    assert_eq!(b1.v, "Total");
    let read: serde_json::Value =
        serde_json::from_str(b1.style.as_deref().expect("style")).expect("json");
    assert_eq!(read["font"]["b"], serde_json::json!(true));
    assert!(b1.fmt.is_none());
}

#[test]
fn a_style_that_is_not_an_ironcalc_style_falls_back_to_the_default() {
    let mut odd = cell("plain");
    odd.style = Some("{not json".to_owned());
    let sheets = SheetModel {
        sheets: vec![Sheet::from_grid("Sheet1", vec![(1, 1, odd)], &[])],
    };
    let back = sheet_model_from_xlsx(&checkpoint_xlsx(&sheets).expect("checkpoint")).expect("read");
    let a1 = at(&back.sheets[0], 1, 1).expect("a1");
    assert_eq!(a1.v, "plain");
    assert!(a1.style.is_none());
}

#[test]
fn an_empty_model_checkpoints_to_a_single_default_sheet() {
    let bytes = checkpoint_xlsx(&SheetModel::default()).expect("checkpoint");
    let back = sheet_model_from_xlsx(&bytes).expect("read");
    assert_eq!(back.sheets.len(), 1);
    assert!(back.sheets[0].cells.is_empty());
}

#[test]
fn bytes_that_are_not_a_workbook_are_a_parse_error() {
    assert!(matches!(
        sheet_model_from_xlsx(b"not a zip"),
        Err(CodecError::Parse { part, .. }) if part == "workbook"
    ));
    let doc = loro::LoroDoc::new();
    assert!(seed_doc_from_xlsx(&doc, b"not a zip").is_err());
}

#[test]
fn seeding_a_loro_doc_from_a_checkpoint_reads_back_the_same_sheets() {
    let bytes = checkpoint_xlsx(&model()).expect("checkpoint");
    let doc = loro::LoroDoc::new();
    assert!(seed_doc_from_xlsx(&doc, &bytes).expect("seed"));
    let seeded = read_sheets(&doc).expect("read doc");
    assert_eq!(seeded.names(), vec!["Budget", "Summary"]);
    let budget = seeded.sheet("Budget").expect("budget");
    assert_eq!(at(budget, 3, 4).map(|c| c.v.as_str()), Some("=B4*2"));
    assert_eq!(budget.merges.len(), 1);
}
