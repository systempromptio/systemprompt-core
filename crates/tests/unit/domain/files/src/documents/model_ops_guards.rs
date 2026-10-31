use std::num::NonZeroUsize;

use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec};
use systemprompt_files::documents::model::ops::apply_rich_ops;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, InsertImage, InsertTable, MarkSpec,
    NodeTarget, SetBlockType, SetMarks, TableDelete, TableInsert, TextBlockType,
};

use super::{SAMPLE_HTML, hash_of, id};

fn html(ops: &[EditOp]) -> Result<String, CodecError> {
    let ids = HtmlCodec.project(SAMPLE_HTML).expect("project");
    apply_rich_ops(DocumentFormat::Html, SAMPLE_HTML, &ids, ops)
        .map(|bytes| String::from_utf8(bytes).expect("utf8"))
}

fn hash(node: &str) -> Option<String> {
    Some(hash_of(
        &HtmlCodec.project(SAMPLE_HTML).expect("project"),
        node,
    ))
}

fn detail(result: Result<String, CodecError>) -> &'static str {
    match result {
        Err(CodecError::Malformed { detail }) => detail,
        other => panic!("expected malformed, got {other:?}"),
    }
}

fn block_type(node: &str, block: TextBlockType) -> EditOp {
    EditOp::SetBlockType(SetBlockType {
        node: NodeTarget::Id(id(node)),
        expected_hash: hash(node),
        block,
    })
}

fn set_marks(
    node: &str,
    find: Option<&str>,
    nth: Option<usize>,
    add: &[(&str, &str)],
    remove: &[&str],
) -> EditOp {
    EditOp::SetMarks(SetMarks {
        node: NodeTarget::Id(id(node)),
        expected_hash: hash(node),
        find: find.map(str::to_owned),
        nth: nth.and_then(NonZeroUsize::new),
        add: add
            .iter()
            .map(|(mark, value)| MarkSpec {
                mark: (*mark).to_owned(),
                value: (!value.is_empty()).then(|| (*value).to_owned()),
            })
            .collect(),
        remove: remove.iter().map(|key| (*key).to_owned()).collect(),
    })
}

fn table_op(row: bool, at: Option<usize>, cells: &[&str]) -> EditOp {
    let op = TableInsert {
        table: NodeTarget::Id(id("body/table[1]")),
        expected_hash: hash("body/table[1]"),
        at,
        cells: cells.iter().map(|cell| (*cell).to_owned()).collect(),
    };
    if row {
        EditOp::TableInsertRow(op)
    } else {
        EditOp::TableInsertCol(op)
    }
}

fn table_delete(row: Option<usize>, col: Option<usize>) -> EditOp {
    EditOp::TableDelete(TableDelete {
        table: NodeTarget::Id(id("body/table[1]")),
        expected_hash: hash("body/table[1]"),
        row,
        col,
    })
}

#[test]
fn block_types_outside_their_range_or_on_non_text_blocks_are_refused() {
    let heading = html(&[block_type("body/p[2]", TextBlockType::Heading { level: 7 })]);
    assert_eq!(detail(heading), "a heading level is 1 to 6");
    let list = html(&[block_type(
        "body/p[2]",
        TextBlockType::ListItem {
            level: 9,
            ordered: true,
        },
    )]);
    assert_eq!(detail(list), "a list level is 0 to 8");
    let table = html(&[block_type("body/table[1]", TextBlockType::Paragraph)]);
    assert_eq!(detail(table), "set_block_type needs a text block");
}

#[test]
fn quotes_code_and_list_items_replace_a_paragraph_in_html() {
    let quoted = html(&[block_type("body/p[2]", TextBlockType::Quote)]).expect("quote");
    assert!(quoted.contains("<blockquote>"));
    assert!(quoted.contains("Second para"));
    let code = html(&[block_type(
        "body/p[1]",
        TextBlockType::Code {
            language: Some("rust".to_owned()),
        },
    )])
    .expect("code");
    assert!(code.contains("<pre"));
    assert!(!code.contains("<strong>bold</strong>"));
    let listed = html(&[block_type(
        "body/p[2]",
        TextBlockType::ListItem {
            level: 0,
            ordered: true,
        },
    )])
    .expect("list");
    assert!(listed.contains("<ol>"));
    let back = html(&[block_type("body/h1[1]", TextBlockType::Paragraph)]).expect("paragraph");
    assert!(back.contains("<p>Heading</p>"));
}

#[test]
fn marks_are_added_to_the_nth_match_removed_and_merged() {
    let out = html(&[set_marks(
        "body/p[1]",
        Some("bold"),
        Some(1),
        &[("italic", "")],
        &["bold"],
    )])
    .expect("apply");
    assert!(out.contains("<em>bold</em>"));
    assert!(!out.contains("<strong>"));

    let missing = html(&[set_marks(
        "body/p[2]",
        Some("para"),
        Some(2),
        &[("bold", "")],
        &[],
    )]);
    assert_eq!(
        detail(missing),
        "set_marks find does not occur in the block"
    );
    let unknown_remove = html(&[set_marks("body/p[2]", None, None, &[], &["comment"])]);
    assert_eq!(
        detail(unknown_remove),
        "set_marks removes an unknown or comment mark"
    );
    let unknown_add = html(&[set_marks("body/p[2]", None, None, &[("sparkle", "")], &[])]);
    assert_eq!(detail(unknown_add), "set_marks adds an unknown mark");
    let valueless = html(&[set_marks("body/p[2]", None, None, &[("color", "")], &[])]);
    assert!(detail(valueless).starts_with("valued marks"));
    let on_table = html(&[set_marks("body/table[1]", None, None, &[("bold", "")], &[])]);
    assert_eq!(detail(on_table), "set_marks needs a text block");

    let coloured = html(&[set_marks(
        "body/p[2]",
        Some("Second"),
        None,
        &[("color", "#ff0000")],
        &[],
    )])
    .expect("colour");
    assert!(coloured.contains("Second"));
    assert!(coloured.to_lowercase().contains("ff0000"));
}

#[test]
fn markdown_refuses_marks_it_cannot_hold() {
    let bytes = b"plain words here\n";
    let ids = MarkdownCodec.project(bytes).expect("project");
    let para = &ids.root[0].children[0];
    let op = EditOp::SetMarks(SetMarks {
        node: NodeTarget::Id(para.id.clone()),
        expected_hash: Some(para.hash8.clone()),
        find: Some("words".to_owned()),
        nth: None,
        add: vec![MarkSpec {
            mark: "highlight".to_owned(),
            value: Some("#ffff00".to_owned()),
        }],
        remove: Vec::new(),
    });
    let error = apply_rich_ops(DocumentFormat::Markdown, bytes, &ids, &[op]).expect_err("refused");
    assert!(matches!(
        error,
        CodecError::UnsupportedOp {
            op: "set_marks",
            format: DocumentFormat::Markdown
        }
    ));
}

#[test]
fn table_edits_outside_the_grid_are_refused() {
    let wide = html(&[table_op(true, None, &["1", "2", "3"])]);
    assert_eq!(
        detail(wide),
        "the row has more cells than the table has columns"
    );
    let far_row = html(&[table_op(true, Some(5), &["1"])]);
    assert_eq!(
        detail(far_row),
        "the row index is past the end of the table"
    );
    let far_col = html(&[table_op(false, Some(5), &["1"])]);
    assert_eq!(
        detail(far_col),
        "the column index is past the end of the table"
    );
    let tall = html(&[table_op(false, None, &["1", "2"])]);
    assert_eq!(
        detail(tall),
        "the column has more cells than the table has rows"
    );
    let both = html(&[table_delete(Some(0), Some(0))]);
    assert_eq!(
        detail(both),
        "table_delete names a row or a column, not both"
    );
    let gone_row = html(&[table_delete(Some(3), None)]);
    assert_eq!(
        detail(gone_row),
        "the row index is past the end of the table"
    );
    let gone_col = html(&[table_delete(None, Some(3))]);
    assert_eq!(
        detail(gone_col),
        "the column index is past the end of the table"
    );
    let not_table = html(&[EditOp::TableInsertRow(TableInsert {
        table: NodeTarget::Id(id("body/p[2]")),
        expected_hash: hash("body/p[2]"),
        at: None,
        cells: vec!["x".to_owned()],
    })]);
    assert_eq!(detail(not_table), "the node is not a table");
}

#[test]
fn whole_table_and_last_row_deletes_remove_the_table_and_later_ops_lose_their_block() {
    let whole = html(&[table_delete(None, None)]).expect("whole");
    assert!(!whole.contains("<table>"));
    let last_row = html(&[table_delete(Some(0), None)]).expect("row");
    assert!(!last_row.contains("<table>"));
    let orphaned = html(&[table_delete(None, None), table_op(true, None, &["a", "b"])]);
    assert_eq!(
        detail(orphaned),
        "an earlier op in the batch removed this op's block"
    );
}

#[test]
fn inserted_tables_pad_ragged_rows_and_images_need_a_blob() {
    let ragged = html(&[EditOp::InsertTable(InsertTable {
        after: NodeTarget::Id(id("body/p[2]")),
        rows: vec![vec!["a".to_owned(), "b".to_owned()], vec!["c".to_owned()]],
    })])
    .expect("ragged");
    assert!(ragged.contains("<tr><td>c</td><td></td></tr>"));
    let empty = html(&[EditOp::InsertTable(InsertTable {
        after: NodeTarget::Id(id("body/p[2]")),
        rows: vec![Vec::new()],
    })]);
    assert_eq!(detail(empty), "insert_table needs at least one cell");
    let blobless = html(&[EditOp::InsertImage(InsertImage {
        after: NodeTarget::Id(id("body/p[2]")),
        blob: String::new(),
        alt: "nothing".to_owned(),
    })]);
    assert_eq!(detail(blobless), "insert_image needs a blob");
    let unsupported = html(&[EditOp::AppendMarkdown {
        markdown: "x".to_owned(),
    }]);
    assert!(matches!(
        unsupported,
        Err(CodecError::UnsupportedOp {
            op: "append_markdown",
            ..
        })
    ));
}
