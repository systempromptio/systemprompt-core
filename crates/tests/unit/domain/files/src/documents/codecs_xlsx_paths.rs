use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
use systemprompt_provider_contracts::documents::{
    CellValue, CodecError, DocumentCodec, EditOp, NodeKind, NodeTarget,
};

use super::{delete, hash_of, id, replace};

fn fixture() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("Fish & <Chips>");
    sheet.cell_mut("B1").set_value_string("\"quoted\" 'single'");
    sheet.cell_mut("A2").set_value_string("apples");
    sheet.cell_mut("B2").set_value_number(3);
    sheet.cell_mut("A3").set_value_string("pears");
    sheet.cell_mut("B3").set_value_number(4);
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut out).expect("write");
    out
}

fn read(bytes: &[u8]) -> umya_spreadsheet::Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).expect("read")
}

fn apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let base = XlsxCodec.project(bytes).expect("project");
    XlsxCodec.apply(bytes, &base, ops)
}

fn value(bytes: &[u8], cell: &str) -> String {
    read(bytes).sheet(0).expect("sheet").value(cell)
}

#[test]
fn project_range_keeps_only_rows_and_cells_inside() {
    let rows = XlsxCodec::project_range(&fixture(), "Sheet1", "B2:B3").expect("range");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id.as_str(), "sheet[Sheet1]/row[2]");
    assert_eq!(rows[0].children.len(), 1);
    assert_eq!(rows[0].children[0].id.as_str(), "sheet[Sheet1]/B2");
    assert_eq!(rows[1].text.as_deref(), Some("\t4"));
}

#[test]
fn project_range_refuses_unknown_sheet_and_bad_range() {
    let missing = XlsxCodec::project_range(&fixture(), "Nope", "A1:B2").expect_err("unknown sheet");
    assert!(matches!(missing, CodecError::UnknownNode { node } if node.as_str() == "sheet[Nope]"));
    let bad = XlsxCodec::project_range(&fixture(), "Sheet1", "not a range").expect_err("bad range");
    assert!(matches!(bad, CodecError::Malformed { .. }));
}

#[test]
fn plain_text_lists_each_sheet_and_row() {
    let text = XlsxCodec.plain_text(&fixture()).expect("text");
    assert!(text.starts_with("# Sheet1\n"));
    assert!(text.contains("apples\t3\n"));
    assert!(text.contains("pears\t4\n"));
}

#[test]
fn preview_html_escapes_markup() {
    let html = XlsxCodec.preview_html(&fixture()).expect("html");
    assert!(html.contains("Fish &amp; &lt;Chips&gt;"));
    assert!(html.contains("&quot;quoted&quot; &#39;single&#39;"));
}

#[test]
fn garbage_bytes_are_a_parse_error() {
    let err = XlsxCodec.project(b"not a zip").expect_err("parse");
    assert!(matches!(err, CodecError::Parse { ref part, .. } if part == "workbook"));
    assert!(XlsxCodec.plain_text(b"junk").is_err());
}

#[test]
fn many_cells_truncate_the_sheet_projection() {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    for row in 1..=210u32 {
        for col in 1..=10u32 {
            sheet.cell_mut((col, row)).set_value_number(f64::from(row));
        }
    }
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut out).expect("write");
    let projection = XlsxCodec.project(&out).expect("project");
    let sheet = &projection.root[0];
    assert_eq!(
        sheet.attrs.get("truncated").map(String::as_str),
        Some("true")
    );
    assert!(sheet.children.last().expect("row").children.is_empty());
    assert_eq!(sheet.children[0].children.len(), 10);
}

#[test]
fn delete_cell_and_row_clear_values() {
    let bytes = fixture();
    let base = XlsxCodec.project(&bytes).expect("project");
    let out = apply(
        &bytes,
        &[
            delete("sheet[Sheet1]/B2", hash_of(&base, "sheet[Sheet1]/B2")),
            delete(
                "sheet[Sheet1]/row[3]",
                hash_of(&base, "sheet[Sheet1]/row[3]"),
            ),
        ],
    )
    .expect("apply");
    assert_eq!(value(&out, "B2"), "");
    assert_eq!(value(&out, "A2"), "apples");
    assert_eq!(value(&out, "A3"), "");
    assert_eq!(value(&out, "B3"), "");
}

#[test]
fn sheet_targets_refuse_replace_and_delete() {
    let bytes = fixture();
    let base = XlsxCodec.project(&bytes).expect("project");
    let hash = hash_of(&base, "sheet[Sheet1]");
    for op in [
        replace("sheet[Sheet1]", hash.clone(), "x"),
        delete("sheet[Sheet1]", hash.clone()),
    ] {
        let err = apply(&bytes, &[op]).expect_err("sheet target");
        assert!(matches!(err, CodecError::UnsupportedOp { .. }));
    }
}

#[test]
fn delete_with_stale_hash_is_refused() {
    let err = apply(
        &fixture(),
        &[delete("sheet[Sheet1]/A2", "00000000".to_owned())],
    )
    .expect_err("stale");
    assert!(matches!(err, CodecError::StaleAnchor { .. }));
}

#[test]
fn unknown_sheet_and_unparseable_node_are_unknown() {
    let bytes = fixture();
    for node in ["sheet[Ghost]/A1", "garbage"] {
        let err = apply(&bytes, &[delete(node, "00000000".to_owned())]).expect_err("unknown");
        assert!(
            matches!(err, CodecError::UnknownNode { .. }),
            "{node}: {err:?}"
        );
    }
}

#[test]
fn str_replace_rewrites_a_unique_string_cell() {
    let out = apply(
        &fixture(),
        &[EditOp::StrReplace {
            old_str: "pears".to_owned(),
            new_str: "plums".to_owned(),
        }],
    )
    .expect("apply");
    assert_eq!(value(&out, "A3"), "plums");
}

#[test]
fn str_replace_refusals() {
    let bytes = fixture();
    let empty = apply(
        &bytes,
        &[EditOp::StrReplace {
            old_str: String::new(),
            new_str: "x".to_owned(),
        }],
    )
    .expect_err("empty");
    assert!(matches!(empty, CodecError::Malformed { .. }));
    let none = apply(
        &bytes,
        &[EditOp::StrReplace {
            old_str: "kiwi".to_owned(),
            new_str: "x".to_owned(),
        }],
    )
    .expect_err("none");
    assert!(matches!(none, CodecError::NoMatch));
    let many = apply(
        &bytes,
        &[EditOp::StrReplace {
            old_str: "p".to_owned(),
            new_str: "x".to_owned(),
        }],
    )
    .expect_err("many");
    assert!(matches!(many, CodecError::AmbiguousMatch { .. }));
}

#[test]
fn whole_document_ops_are_unsupported() {
    let err = apply(
        &fixture(),
        &[EditOp::SetText {
            text: "x".to_owned(),
        }],
    )
    .expect_err("unsupported");
    assert!(matches!(
        err,
        CodecError::UnsupportedOp { op: "set_text", .. }
    ));
}

#[test]
fn set_cells_on_missing_sheet_is_unknown() {
    let err = apply(
        &fixture(),
        &[EditOp::SetCells {
            sheet: "Ghost".to_owned(),
            range: "A1".to_owned(),
            values: vec![vec![CellValue::Text("x".to_owned())]],
            expected_hash: None,
        }],
    )
    .expect_err("ghost");
    assert!(matches!(err, CodecError::UnknownNode { .. }));
}

#[test]
fn bracketed_sheet_names_round_trip_through_node_ids() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).expect("sheet").set_name("Q[1]");
    book.sheet_mut(0)
        .expect("sheet")
        .cell_mut("A1")
        .set_value_string("old");
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).expect("write");
    let base = XlsxCodec.project(&bytes).expect("project");
    assert_eq!(base.root[0].id.as_str(), "sheet[Q[1\\]]");
    let out = XlsxCodec
        .apply(
            &bytes,
            &base,
            &[replace(
                "sheet[Q[1\\]]/A1",
                hash_of(&base, "sheet[Q[1\\]]/A1"),
                "new",
            )],
        )
        .expect("apply");
    assert_eq!(read(&out).sheet(0).expect("sheet").value("A1"), "new");
}

#[test]
fn pending_insert_after_a_row_writes_a_marker() {
    let bytes = fixture();
    let out = apply(
        &bytes,
        &[EditOp::InsertAfter {
            node: NodeTarget::Id(id("sheet[Sheet1]/row[2]")),
            kind: NodeKind::Pending,
            text: "fill totals".to_owned(),
            content_as: Default::default(),
        }],
    )
    .expect("apply");
    let projection = XlsxCodec.project(&out).expect("project");
    let pending = projection
        .find(&id("sheet[Sheet1]/A3"))
        .expect("inserted cell");
    assert_eq!(pending.kind, NodeKind::Pending);
    assert_eq!(pending.text.as_deref(), Some("fill totals"));
    assert_eq!(value(&out, "A4"), "pears");
}
