use std::collections::BTreeMap;

use systemprompt_files::documents::codecs::xlsx::{XlsxCodec, blank_xlsx};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CellValue, CodecError, ContentAs, DocumentCodec, EditOp, NodeKind, NodeTarget, Projection,
    TemplateParams, hash8,
};

fn fixture() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("Name");
    sheet.cell_mut("B1").set_value_string("Qty");
    sheet.cell_mut("A2").set_value_string("apples");
    sheet.cell_mut("B2").set_value_number(3);
    sheet.cell_mut("A3").set_value_string("pears");
    sheet.cell_mut("B3").set_value_number(4);
    sheet.cell_mut("B4").set_formula("SUM(B2:B3)");
    sheet.cell_mut("C2").style_mut().font_mut().set_bold(true);
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut out).expect("write");
    out
}

fn read(bytes: &[u8]) -> umya_spreadsheet::Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).expect("read")
}

fn project(bytes: &[u8]) -> Projection {
    XlsxCodec.project(bytes).expect("project")
}

fn apply(bytes: &[u8], ops: Vec<EditOp>) -> Result<Vec<u8>, CodecError> {
    let base = project(bytes);
    XlsxCodec.apply(bytes, &base, &ops)
}

fn node(id: &str) -> DocumentNodeId {
    DocumentNodeId::new(id)
}

#[test]
fn projection_ids_and_hashes() {
    let projection = project(&fixture());
    let sheet = &projection.root[0];
    assert_eq!(sheet.id.as_str(), "sheet[Sheet1]");
    assert_eq!(sheet.kind, NodeKind::Sheet);
    assert_eq!(sheet.attrs.get("index").map(String::as_str), Some("0"));
    let row = projection
        .find(&node("sheet[Sheet1]/row[2]"))
        .expect("row 2");
    assert_eq!(row.text.as_deref(), Some("apples\t3"));
    assert_eq!(row.hash8, hash8("apples\t3"));
    let cell = projection.find(&node("sheet[Sheet1]/B2")).expect("B2");
    assert_eq!(cell.hash8, hash8("3"));
    assert_eq!(cell.attrs.get("type").map(String::as_str), Some("n"));
    let formula = projection.find(&node("sheet[Sheet1]/B4")).expect("B4");
    assert_eq!(
        formula.attrs.get("formula").map(String::as_str),
        Some("SUM(B2:B3)")
    );
}

#[test]
fn set_cells_with_stale_hash_reports_current_text() {
    let err = apply(
        &fixture(),
        vec![EditOp::SetCells {
            sheet: "Sheet1".to_owned(),
            range: "A2:B2".to_owned(),
            values: vec![vec![
                CellValue::Text("x".to_owned()),
                CellValue::Number(1.0),
            ]],
            expected_hash: Some("00000000".to_owned()),
        }],
    )
    .expect_err("stale");
    match err {
        CodecError::StaleAnchor {
            current_text,
            actual,
            ..
        } => {
            assert_eq!(current_text, "apples\t3");
            assert_eq!(actual, hash8("apples\t3"));
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn set_cells_writes_typed_values_and_keeps_style() {
    let bytes = fixture();
    let out = apply(
        &bytes,
        vec![
            EditOp::SetCells {
                sheet: "Sheet1".to_owned(),
                range: "A5:D5".to_owned(),
                values: vec![vec![
                    CellValue::Text("total".to_owned()),
                    CellValue::Number(7.5),
                    CellValue::Bool(true),
                    CellValue::Text("=B5*2".to_owned()),
                ]],
                expected_hash: Some(hash8("\t\t\t")),
            },
            EditOp::SetCells {
                sheet: "Sheet1".to_owned(),
                range: "C2".to_owned(),
                values: vec![vec![CellValue::Text("bold".to_owned())]],
                expected_hash: None,
            },
        ],
    )
    .expect("apply");
    let book = read(&out);
    let sheet = book.sheet(0).expect("sheet");
    assert_eq!(sheet.cell("A5").expect("A5").value(), "total");
    assert_eq!(sheet.cell("B5").expect("B5").value_number(), Some(7.5));
    assert_eq!(sheet.cell("C5").expect("C5").data_type(), "b");
    assert_eq!(sheet.cell("D5").expect("D5").formula(), "B5*2");
    let c2 = sheet.cell("C2").expect("C2");
    assert_eq!(c2.value(), "bold");
    assert!(c2.style().font().expect("font").bold());
}

#[test]
fn set_cells_shape_mismatch_is_malformed() {
    let err = apply(
        &fixture(),
        vec![EditOp::SetCells {
            sheet: "Sheet1".to_owned(),
            range: "A1:C1".to_owned(),
            values: vec![vec![CellValue::Empty]],
            expected_hash: None,
        }],
    )
    .expect_err("shape");
    assert!(matches!(err, CodecError::Malformed { .. }));
}

#[test]
fn str_replace_unique_and_ambiguous() {
    let bytes = fixture();
    let out = apply(
        &bytes,
        vec![EditOp::StrReplace {
            old_str: "pears".to_owned(),
            new_str: "plums".to_owned(),
        }],
    )
    .expect("unique");
    assert_eq!(
        read(&out)
            .sheet(0)
            .expect("sheet")
            .cell("A3")
            .expect("A3")
            .value(),
        "plums"
    );
    let err = apply(
        &bytes,
        vec![EditOp::StrReplace {
            old_str: "a".to_owned(),
            new_str: "o".to_owned(),
        }],
    )
    .expect_err("ambiguous");
    assert!(matches!(err, CodecError::AmbiguousMatch { .. }));
}

#[test]
fn insert_after_adds_a_row() {
    let out = apply(
        &fixture(),
        vec![EditOp::InsertAfter {
            node: NodeTarget::Id(node("sheet[Sheet1]/row[2]")),
            kind: NodeKind::Row,
            text: "kiwis\t9".to_owned(),
            content_as: ContentAs::Text,
        }],
    )
    .expect("insert");
    let projection = project(&out);
    let row = projection
        .find(&node("sheet[Sheet1]/row[3]"))
        .expect("row 3");
    assert_eq!(row.text.as_deref(), Some("kiwis\t9"));
    let moved = projection
        .find(&node("sheet[Sheet1]/row[4]"))
        .expect("row 4");
    assert_eq!(moved.text.as_deref(), Some("pears\t4"));
}

#[test]
fn preview_has_table_and_caption() {
    let html = XlsxCodec.preview_html(&fixture()).expect("preview");
    assert!(html.contains("<table>"));
    assert!(html.contains("<caption>Sheet1</caption>"));
    assert!(html.contains("<td>apples</td>"));
}

#[test]
fn blank_projects_to_one_empty_sheet() {
    let projection = project(&blank_xlsx().expect("blank"));
    assert_eq!(projection.root.len(), 1);
    assert_eq!(projection.root[0].id.as_str(), "sheet[Sheet1]");
    assert!(projection.root[0].children.is_empty());
}

#[test]
fn instantiate_fills_placeholder() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .expect("sheet")
        .cell_mut("A1")
        .set_value_string("Hello {{name}}");
    let mut template = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut template).expect("write");
    let params = TemplateParams {
        values: BTreeMap::from([("name".to_owned(), "Ada".to_owned())]),
    };
    let out = XlsxCodec
        .instantiate(&template, &params)
        .expect("instantiate");
    assert_eq!(
        read(&out)
            .sheet(0)
            .expect("sheet")
            .cell("A1")
            .expect("A1")
            .value(),
        "Hello Ada"
    );
}
