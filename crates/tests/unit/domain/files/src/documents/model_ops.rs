use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec};
use systemprompt_files::documents::model::ops::apply_rich_ops;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, InsertImage, InsertTable, MarkSpec,
    NodeTarget, SetBlockType, SetMarks, TableDelete, TableInsert, TextBlockType,
};

use super::docx_fixture::{fixture, part, project};
use super::{SAMPLE_HTML, hash_of, id};

fn html(ops: &[EditOp]) -> Result<String, CodecError> {
    let ids = HtmlCodec.project(SAMPLE_HTML).expect("project");
    apply_rich_ops(DocumentFormat::Html, SAMPLE_HTML, &ids, ops)
        .map(|bytes| String::from_utf8(bytes).expect("utf8"))
}

fn marks(node: &str, hash: String, find: Option<&str>, add: &[(&str, Option<&str>)]) -> EditOp {
    EditOp::SetMarks(SetMarks {
        node: NodeTarget::Id(id(node)),
        expected_hash: Some(hash),
        find: find.map(str::to_owned),
        nth: None,
        add: add
            .iter()
            .map(|(mark, value)| MarkSpec {
                mark: (*mark).to_owned(),
                value: value.map(str::to_owned),
            })
            .collect(),
        remove: Vec::new(),
    })
}

fn sample_hash(node: &str) -> String {
    hash_of(&HtmlCodec.project(SAMPLE_HTML).expect("project"), node)
}

fn table_insert(node: &str, at: Option<usize>, cells: &[&str]) -> TableInsert {
    TableInsert {
        table: NodeTarget::Id(id(node)),
        expected_hash: Some(sample_hash(node)),
        at,
        cells: cells.iter().map(|cell| (*cell).to_owned()).collect(),
    }
}

#[test]
fn set_marks_bolds_a_quoted_span_and_keeps_the_document_around_it() {
    let out = html(&[marks(
        "body/p[2]",
        sample_hash("body/p[2]"),
        Some("para"),
        &[("bold", None)],
    )])
    .expect("apply");
    assert!(out.contains("<p>Second <strong>para</strong></p>"));
    assert!(out.contains("<title>Keep Me</title>"));
}

#[test]
fn a_link_needs_an_allowed_scheme() {
    let error = html(&[marks(
        "body/p[2]",
        sample_hash("body/p[2]"),
        None,
        &[("link", Some("javascript:alert(1)"))],
    )])
    .expect_err("refused");
    assert!(error.to_string().contains("bad_mark"));
}

#[test]
fn a_comment_mark_is_refused() {
    let error = html(&[marks(
        "body/p[2]",
        sample_hash("body/p[2]"),
        None,
        &[("comment", Some("t1"))],
    )])
    .expect_err("refused");
    assert!(error.to_string().contains("comment"));
}

#[test]
fn set_block_type_turns_a_paragraph_into_a_heading() {
    let out = html(&[EditOp::SetBlockType(SetBlockType {
        node: NodeTarget::Id(id("body/p[2]")),
        expected_hash: Some(sample_hash("body/p[2]")),
        block: TextBlockType::Heading { level: 2 },
    })])
    .expect("apply");
    assert!(out.contains("<h2>Second para</h2>"));
}

#[test]
fn table_ops_insert_a_row_and_a_column_then_delete_the_first_column() {
    let out = html(&[
        EditOp::TableInsertRow(table_insert("body/table[1]", None, &["c", "d"])),
        EditOp::TableInsertCol(table_insert("body/table[1]", Some(0), &["x", "y"])),
    ])
    .expect("apply");
    assert!(out.contains("<tr><td>x</td><td>a</td><td>b</td></tr>"));
    assert!(out.contains("<tr><td>y</td><td>c</td><td>d</td></tr>"));
    let out = html(&[EditOp::TableDelete(TableDelete {
        table: NodeTarget::Id(id("body/td[1]")),
        expected_hash: Some(sample_hash("body/td[1]")),
        row: None,
        col: Some(0),
    })])
    .expect("apply");
    assert!(out.contains("<tr><td>b</td></tr>"));
}

#[test]
fn deleting_the_last_column_removes_the_table() {
    let delete = |col| {
        EditOp::TableDelete(TableDelete {
            table: NodeTarget::Id(id("body/table[1]")),
            expected_hash: Some(sample_hash("body/table[1]")),
            row: None,
            col: Some(col),
        })
    };
    let out = html(&[delete(1), delete(0)]).expect("apply");
    assert!(!out.contains("<table>"));
}

#[test]
fn insert_table_lands_after_its_anchor() {
    let out = html(&[EditOp::InsertTable(InsertTable {
        after: NodeTarget::Id(id("body/h1[1]")),
        rows: vec![vec!["k".to_owned(), "v".to_owned()]],
    })])
    .expect("apply");
    let heading = out.find("<h1>Heading</h1>").expect("heading");
    let table = out
        .find("<table><tr><td>k</td><td>v</td></tr></table>")
        .expect("table");
    assert!(heading < table);
}

#[test]
fn a_markdown_image_goes_in_after_a_source_line() {
    let source = b"# Title\n\nSome text.\n";
    let ids = MarkdownCodec.project(source).expect("project");
    let op = EditOp::InsertImage(InsertImage {
        after: NodeTarget::Id(id("line[1]")),
        blob: "https://cdn.example.com/chart.png".to_owned(),
        alt: "Chart".to_owned(),
    });
    let out = apply_rich_ops(DocumentFormat::Markdown, source, &ids, &[op]).expect("apply");
    let out = String::from_utf8(out).expect("utf8");
    assert!(
        out.starts_with("# Title\n\n![Chart](https://cdn.example.com/chart.png)\n\nSome text.")
    );
}

#[test]
fn a_docx_image_is_refused() {
    let bytes = fixture();
    let ids = project(&bytes);
    let heading = ids
        .nodes()
        .find(|node| node.text.as_deref() == Some("Quarterly Report"))
        .expect("heading");
    let op = EditOp::InsertImage(InsertImage {
        after: NodeTarget::Id(heading.id.clone()),
        blob: "https://cdn.example.com/chart.png".to_owned(),
        alt: String::new(),
    });
    let error = apply_rich_ops(DocumentFormat::Docx, &bytes, &ids, &[op]).expect_err("refused");
    assert!(matches!(
        error,
        CodecError::UnsupportedOp {
            op: "insert_image",
            ..
        }
    ));
}

#[test]
fn a_docx_link_is_a_real_hyperlink_with_an_external_relationship() {
    let bytes = fixture();
    let ids = project(&bytes);
    let node = ids
        .nodes()
        .find(|node| node.text.as_deref() == Some("Revenue grew strongly."))
        .expect("paragraph");
    let op = marks(
        node.id.as_str(),
        node.hash8.clone(),
        Some("strongly"),
        &[("link", Some("https://example.com/q?a=1&b=2"))],
    );
    let out = apply_rich_ops(DocumentFormat::Docx, &bytes, &ids, &[op]).expect("apply");
    let document = part(&out, "word/document.xml");
    let rels = part(&out, "word/_rels/document.xml.rels");
    assert!(document.contains("<w:hyperlink r:id=\"rIdLink1\">"));
    assert!(rels.contains("Id=\"rIdLink1\""));
    assert!(rels.contains("Target=\"https://example.com/q?a=1&amp;b=2\""));
    assert!(rels.contains("TargetMode=\"External\""));
}

#[test]
fn an_op_on_a_node_the_document_lacks_is_an_unknown_node() {
    let error =
        html(&[marks("body/p[9]", String::new(), None, &[("bold", None)])]).expect_err("refused");
    assert!(matches!(error, CodecError::UnknownNode { .. }));
}
