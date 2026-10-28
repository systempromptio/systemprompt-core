use systemprompt_files::documents::codecs::odf::{OdsCodec, blank_ods};
use systemprompt_provider_contracts::documents::{
    CellValue, CodecError, ContentAs, DocumentCodec, EditOp, NodeTarget,
};

use super::docx_fixture::part;
use super::odf_fixture::{
    apply, assert_mimetype_first_stored, assert_only_content_changed, hash_of, id, package, text_of,
};

const MIME: &str = "application/vnd.oasis.opendocument.spreadsheet";

const BODY: &str = concat!(
    r#"<office:spreadsheet><table:table table:name="Data"><table:table-column table:number-columns-repeated="3"/>"#,
    r#"<table:table-row><table:table-cell office:value-type="string"><text:p>Name</text:p></table:table-cell>"#,
    r#"<table:table-cell office:value-type="float" office:value="42"><text:p>42</text:p></table:table-cell></table:table-row>"#,
    r#"<table:table-row table:number-rows-repeated="2"><table:table-cell table:number-columns-repeated="3" office:value-type="float" office:value="1"><text:p>1</text:p></table:table-cell></table:table-row>"#,
    r#"<table:table-row table:number-rows-repeated="1000"><table:table-cell table:number-columns-repeated="1024"/></table:table-row>"#,
    "</table:table></office:spreadsheet>",
);

fn sheet() -> Vec<u8> {
    package(MIME, BODY)
}

#[test]
fn repeated_rows_and_columns_project_as_virtual_cells() {
    let projection = OdsCodec.project(&sheet()).expect("project");
    let root = projection.find(&id("sheet[Data]")).expect("sheet");
    assert_eq!(root.text.as_deref(), Some("Data"));
    assert_eq!(
        text_of(&projection, "sheet[Data]/A1").as_deref(),
        Some("Name")
    );
    let b1 = projection.find(&id("sheet[Data]/B1")).expect("B1");
    assert_eq!(b1.attrs.get("type").map(String::as_str), Some("float"));
    assert_eq!(b1.attrs.get("value").map(String::as_str), Some("42"));
    for cell in ["A2", "B2", "C2", "A3", "B3", "C3"] {
        assert_eq!(
            text_of(&projection, &format!("sheet[Data]/{cell}")).as_deref(),
            Some("1"),
            "{cell}"
        );
    }
    assert!(projection.find(&id("sheet[Data]/A4")).is_none());
    assert_eq!(
        projection
            .nodes()
            .filter(|n| n.id.as_str().contains("/row["))
            .count(),
        3
    );
}

#[test]
fn set_cells_splits_repeated_runs_and_appends_past_the_end() {
    let bytes = sheet();
    let ops = [
        EditOp::SetCells {
            sheet: "Data".into(),
            range: "B2".into(),
            values: vec![vec![CellValue::Text("x".into())]],
            expected_hash: None,
        },
        EditOp::SetCells {
            sheet: "Data".into(),
            range: "E1010:E1010".into(),
            values: vec![vec![CellValue::Number(7.0)]],
            expected_hash: None,
        },
        EditOp::SetCells {
            sheet: "Data".into(),
            range: "C1".into(),
            values: vec![vec![CellValue::Text("=SUM([.A2:.B2])".into())]],
            expected_hash: None,
        },
    ];
    let out = apply(&OdsCodec, &bytes, &ops);
    assert_only_content_changed(&bytes, &out);
    let next = OdsCodec.project(&out).expect("reproject");
    assert_eq!(text_of(&next, "sheet[Data]/B2").as_deref(), Some("x"));
    for cell in ["A2", "C2", "A3", "B3", "C3"] {
        assert_eq!(
            text_of(&next, &format!("sheet[Data]/{cell}")).as_deref(),
            Some("1"),
            "{cell}"
        );
    }
    assert_eq!(text_of(&next, "sheet[Data]/E1010").as_deref(), Some("7"));
    let c1 = next.find(&id("sheet[Data]/C1")).expect("C1");
    assert_eq!(
        c1.attrs.get("formula").map(String::as_str),
        Some("of:=SUM([.A2:.B2])")
    );
    assert!(
        part(&out, "content.xml").contains(r#"table:number-rows-repeated="1000""#)
            || part(&out, "content.xml").contains("table:number-rows-repeated=\"9")
    );
}

#[test]
fn replace_text_on_a_cell_checks_its_hash() {
    let bytes = sheet();
    let base = OdsCodec.project(&bytes).expect("project");
    let stale = OdsCodec.apply(
        &bytes,
        &base,
        &[EditOp::ReplaceText {
            node: NodeTarget::Id(id("sheet[Data]/A1")),
            expected_hash: Some("00000000".into()),
            text: "x".into(),
            content_as: ContentAs::Text,
        }],
    );
    assert!(matches!(stale, Err(CodecError::StaleAnchor { .. })));
    let op = EditOp::ReplaceText {
        node: NodeTarget::Id(id("sheet[Data]/A1")),
        expected_hash: Some(hash_of(&base, "sheet[Data]/A1")),
        text: "Label".into(),
        content_as: ContentAs::Text,
    };
    let out = OdsCodec.apply(&bytes, &base, &[op]).expect("apply");
    let next = OdsCodec.project(&out).expect("reproject");
    assert_eq!(text_of(&next, "sheet[Data]/A1").as_deref(), Some("Label"));
    assert_eq!(text_of(&next, "sheet[Data]/B1").as_deref(), Some("42"));
    let delete = OdsCodec.apply(
        &bytes,
        &base,
        &[EditOp::Delete {
            node: NodeTarget::Id(id("sheet[Data]/A1")),
            expected_hash: Some(hash_of(&base, "sheet[Data]/A1")),
        }],
    );
    assert!(matches!(delete, Err(CodecError::UnsupportedOp { .. })));
}

#[test]
fn large_repeated_blocks_are_capped() {
    let body = concat!(
        r#"<office:spreadsheet><table:table table:name="Big">"#,
        r#"<table:table-row table:number-rows-repeated="100"><table:table-cell table:number-columns-repeated="100" office:value-type="float" office:value="5"><text:p>5</text:p></table:table-cell></table:table-row>"#,
        "</table:table></office:spreadsheet>",
    );
    let projection = OdsCodec.project(&package(MIME, body)).expect("project");
    let root = projection.find(&id("sheet[Big]")).expect("sheet");
    assert_eq!(
        root.attrs.get("truncated").map(String::as_str),
        Some("true")
    );
    let cells = projection
        .nodes()
        .filter(|n| n.kind == systemprompt_provider_contracts::documents::NodeKind::Cell)
        .count();
    assert_eq!(cells, 2_000);
}

#[test]
fn preview_and_blank() {
    let preview = OdsCodec.preview_html(&sheet()).expect("preview");
    assert!(preview.contains("<caption>Data</caption>"));
    assert!(preview.contains("<td>Name</td><td>42</td>"));
    let blank = blank_ods().expect("blank");
    assert_mimetype_first_stored(&blank);
    let projection = OdsCodec.project(&blank).expect("project");
    assert!(projection.find(&id("sheet[Sheet1]")).is_some());
}
