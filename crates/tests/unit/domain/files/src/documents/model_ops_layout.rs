use serde_json::json;
use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec};
use systemprompt_files::documents::model::ops::apply_rich_ops;
use systemprompt_provider_contracts::documents::{
    CellUpdate, CodecError, DocumentCodec, DocumentFormat, EditOp, InsertBlock, NodeKind,
    NodeTarget, Projection, ProjectionNode, SetParagraph, UpdateTable,
};

use super::docx_fixture::{fixture, part, project};
use super::{SAMPLE_HTML, SAMPLE_MD};

fn target(ids: &Projection, pick: impl Fn(&ProjectionNode) -> bool) -> NodeTarget {
    NodeTarget::Id(
        ids.nodes()
            .find(|node| pick(node))
            .expect("node")
            .id
            .clone(),
    )
}

fn by_text(ids: &Projection, text: &str) -> NodeTarget {
    target(ids, |node| node.text.as_deref() == Some(text))
}

fn table(ids: &Projection) -> NodeTarget {
    target(ids, |node| node.kind == NodeKind::Table)
}

fn docx(ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let bytes = fixture();
    apply_rich_ops(DocumentFormat::Docx, &bytes, &project(&bytes), ops)
}

fn docx_document(ops: &[EditOp]) -> String {
    part(&docx(ops).expect("apply"), "word/document.xml")
}

fn paragraph(attrs: serde_json::Value) -> EditOp {
    let bytes = fixture();
    EditOp::SetParagraph(SetParagraph {
        node: by_text(&project(&bytes), "Revenue grew strongly."),
        expected_hash: None,
        attrs,
    })
}

fn update(cells: Vec<CellUpdate>) -> UpdateTable {
    let bytes = fixture();
    UpdateTable {
        table: table(&project(&bytes)),
        expected_hash: None,
        cells,
        widths: None,
        header_rows: None,
        style: None,
    }
}

#[test]
fn set_paragraph_writes_alignment_and_spacing_into_the_docx() {
    let xml = docx_document(&[paragraph(json!({"align": "center", "space_after": 240}))]);
    assert!(xml.contains("w:jc w:val=\"center\""), "{xml}");
    assert!(xml.contains("w:after=\"4800\""), "{xml}");
}

#[test]
fn set_paragraph_with_a_null_clears_the_attribute() {
    let ops = [
        paragraph(json!({"align": "right"})),
        paragraph(json!({"align": null})),
    ];
    let xml = docx_document(&ops);
    assert!(!xml.contains("w:jc w:val=\"right\""));
}

#[test]
fn set_paragraph_refuses_attrs_that_are_not_an_object() {
    let error = docx(&[paragraph(json!(["align"]))]).expect_err("refused");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn set_paragraph_refuses_an_unknown_alignment_as_a_payload_error() {
    let error = docx(&[paragraph(json!({"align": "diagonal"}))]).expect_err("refused");
    assert!(
        matches!(
            error,
            CodecError::Payload {
                what: "set_paragraph.attrs",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn set_paragraph_on_a_table_is_refused_as_not_a_text_block() {
    let bytes = fixture();
    let op = EditOp::SetParagraph(SetParagraph {
        node: table(&project(&bytes)),
        expected_hash: None,
        attrs: json!({"align": "left"}),
    });
    let error = docx(&[op]).expect_err("refused");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn layout_ops_are_unsupported_on_html() {
    let ids = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let node = target(&ids, |node| node.kind == NodeKind::Paragraph);
    let op = EditOp::SetParagraph(SetParagraph {
        node,
        expected_hash: None,
        attrs: json!({"align": "left"}),
    });
    let error = apply_rich_ops(DocumentFormat::Html, SAMPLE_HTML, &ids, &[op]).expect_err("html");
    assert!(
        matches!(
            error,
            CodecError::UnsupportedOp {
                op: "set_paragraph",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn insert_block_adds_a_quote_after_its_anchor() {
    let bytes = fixture();
    let op = EditOp::InsertBlock(InsertBlock {
        after: by_text(&project(&bytes), "Quarterly Report"),
        block: json!({"type": "quote", "text": "Inserted quotation", "id": "spoofed"}),
    });
    let out = docx(&[op]).expect("apply");
    let ids = project(&out);
    let texts: Vec<_> = ids.nodes().filter_map(|node| node.text.clone()).collect();
    let heading = texts
        .iter()
        .position(|t| t == "Quarterly Report")
        .expect("heading");
    assert_eq!(
        texts.get(heading + 1).map(String::as_str),
        Some("Inserted quotation")
    );
}

#[test]
fn insert_block_refuses_raw_markup_even_nested_in_a_table() {
    let bytes = fixture();
    let anchor = by_text(&project(&bytes), "Quarterly Report");
    let raw = json!({"type": "raw", "source": "<w:p/>"});
    let nested = json!({
        "type": "table",
        "rows": [[{"blocks": [{"id": "", "type": "raw", "source": "<w:p/>"}]}]]
    });
    for block in [raw, nested] {
        let op = EditOp::InsertBlock(InsertBlock {
            after: anchor.clone(),
            block,
        });
        let error = docx(&[op]).expect_err("raw refused");
        assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
    }
}

#[test]
fn insert_block_refuses_a_malformed_block_as_a_payload_error() {
    let bytes = fixture();
    let op = EditOp::InsertBlock(InsertBlock {
        after: by_text(&project(&bytes), "Quarterly Report"),
        block: json!({"type": "no_such_block"}),
    });
    let error = docx(&[op]).expect_err("refused");
    assert!(
        matches!(
            error,
            CodecError::Payload {
                what: "insert_block.block",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn update_table_rewrites_a_cell_and_shades_it() {
    let mut op = update(vec![CellUpdate {
        row: 0,
        col: 1,
        text: Some("Grand total".to_owned()),
        props: Some(json!({"shade": "FFEE00"})),
    }]);
    op.widths = Some(vec![Some(100), None]);
    op.header_rows = Some(1);
    op.style = Some("TableGrid".to_owned());
    let xml = docx_document(&[EditOp::UpdateTable(op)]);
    assert!(xml.contains("Grand total"));
    assert!(xml.contains("FFEE00"), "{xml}");
    assert!(!xml.contains(">Total<"));
}

#[test]
fn update_table_refuses_a_cell_outside_the_grid() {
    let op = update(vec![CellUpdate {
        row: 5,
        col: 0,
        text: Some("x".to_owned()),
        props: None,
    }]);
    let error = docx(&[EditOp::UpdateTable(op)]).expect_err("refused");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn update_table_on_a_paragraph_is_refused() {
    let bytes = fixture();
    let mut op = update(Vec::new());
    op.table = by_text(&project(&bytes), "First point");
    let error = docx(&[EditOp::UpdateTable(op)]).expect_err("refused");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn set_paragraph_is_accepted_on_markdown_when_it_can_hold_it() {
    let ids = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let node = target(&ids, |node| node.kind == NodeKind::Paragraph);
    let op = EditOp::SetParagraph(SetParagraph {
        node,
        expected_hash: None,
        attrs: json!({}),
    });
    let out = apply_rich_ops(DocumentFormat::Markdown, SAMPLE_MD, &ids, &[op]).expect("noop attrs");
    assert!(!out.is_empty());
}
