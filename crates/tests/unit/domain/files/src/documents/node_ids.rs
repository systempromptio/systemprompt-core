//! Stable node ids: minting, carry-forward across edits in every format,
//! change diffs between revisions and expressing an upload as ops.

use std::collections::HashSet;

use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::pptx::PptxCodec;
use systemprompt_files::documents::codecs::xlsx::XlsxCodec;
use systemprompt_files::documents::codecs::{HtmlCodec, MarkdownCodec};
use systemprompt_files::documents::models::NodeIdEntry;
use systemprompt_files::documents::nodes::{
    ChangeItem, NodeIdMinter, carry_forward, changes, derive_ops, mint_all, with_ids, writer_tag,
};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    DocumentCodec, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
};

use super::{SAMPLE_HTML, SAMPLE_MD, delete, hash_of, insert_after, replace};

fn minter(writer: &str) -> NodeIdMinter {
    NodeIdMinter::new(writer.to_owned(), 0)
}

fn identified(projection: &Projection) -> Projection {
    let entries = mint_all(projection, &mut minter("a"));
    with_ids(projection.clone(), &entries)
}

struct Step {
    before: Projection,
    after: Projection,
    entries: Vec<NodeIdEntry>,
}

fn step(codec: &dyn DocumentCodec, bytes: &[u8], ops: &[EditOp]) -> Step {
    let paths = codec.project(bytes).expect("project");
    let before = identified(&paths);
    let out = codec.apply(bytes, &paths, ops).expect("apply");
    let next = codec.project(&out).expect("reproject");
    let retired: HashSet<DocumentNodeId> = ops
        .iter()
        .filter_map(|op| match op {
            EditOp::Delete { node, .. } => node.id().cloned(),
            _ => None,
        })
        .filter_map(|path| id_at(&before, path.as_str()))
        .collect();
    let entries = carry_forward(&before, &next, &retired, &mut minter("b"));
    let after = with_ids(next, &entries);
    Step {
        before,
        after,
        entries,
    }
}

fn id_at(projection: &Projection, path: &str) -> Option<DocumentNodeId> {
    projection
        .nodes()
        .find(|node| node.attrs.get("path").map(String::as_str) == Some(path))
        .map(|node| node.id.clone())
}

fn by_text<'p>(projection: &'p Projection, text: &str) -> &'p ProjectionNode {
    projection
        .nodes()
        .find(|node| node.children.is_empty() && node.text.as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no block {text}"))
}

fn unchanged_keep_ids(step: &Step) {
    for node in step.after.nodes() {
        let Some(old) = step.before.find(&node.id) else {
            continue;
        };
        assert_eq!(old.kind, node.kind, "id {} changed kind", node.id);
    }
    let kept = step
        .after
        .nodes()
        .filter(|node| step.before.find(&node.id).is_some())
        .count();
    assert!(kept > 0, "no id carried");
    let unique: HashSet<&DocumentNodeId> = step.entries.iter().map(|e| &e.node_id).collect();
    assert_eq!(unique.len(), step.entries.len(), "ids are unique");
}

#[test]
fn writer_tags_are_short_base36_and_stable() {
    let tag = writer_tag("session-1");
    assert_eq!(tag, writer_tag("session-1"));
    assert_ne!(tag, writer_tag("session-2"));
    assert!(tag.len() <= 8 && !tag.is_empty(), "{tag}");
    assert!(
        tag.chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    );
    let mut minter = NodeIdMinter::new(tag.clone(), 41);
    assert_eq!(minter.mint().as_str(), format!("{tag}.42"));
}

#[test]
fn html_ids_survive_an_insert_above() {
    let base = HtmlCodec.project(SAMPLE_HTML).expect("project");
    let first = base.nodes().nth(1).expect("a node").id.to_string();
    let s = step(
        &HtmlCodec,
        SAMPLE_HTML,
        &[insert_after(&first, NodeKind::Paragraph, "inserted")],
    );
    unchanged_keep_ids(&s);
    let new = by_text(&s.after, "inserted");
    assert!(s.before.find(&new.id).is_none());
    assert!(new.id.as_str().starts_with("b."));
}

#[test]
fn markdown_ids_survive_a_delete_and_the_deleted_id_retires() {
    let base = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let target = base
        .nodes()
        .find(|node| node.children.is_empty() && node.text.is_some())
        .expect("a line")
        .id
        .to_string();
    let s = step(
        &MarkdownCodec,
        SAMPLE_MD,
        &[delete(&target, hash_of(&base, &target))],
    );
    unchanged_keep_ids(&s);
    let gone = id_at(&s.before, &target).expect("id of the deleted line");
    assert!(s.after.find(&gone).is_none(), "{gone} retired");
}

#[test]
fn docx_a_replaced_paragraph_keeps_its_id() {
    let bytes = super::docx_fixture::fixture();
    let s = step(
        &DocxCodec,
        &bytes,
        &[replace(
            "pid:2B3C4D5E",
            super::docx_fixture::hash_of(&DocxCodec.project(&bytes).expect("p"), "pid:2B3C4D5E"),
            "rewritten",
        )],
    );
    unchanged_keep_ids(&s);
    let was = id_at(&s.before, "pid:2B3C4D5E").expect("id");
    assert_eq!(by_text(&s.after, "rewritten").id, was);
}

#[test]
fn pptx_ids_survive_an_inserted_slide() {
    let deck = super::pptx_fixture::two_slide_deck();
    let s = step(
        &PptxCodec,
        &deck,
        &[EditOp::InsertSlide {
            after: Some(DocumentNodeId::new("slide[1]")),
            layout: None,
            title: Some("Inserted".to_owned()),
            body: Vec::new(),
        }],
    );
    unchanged_keep_ids(&s);
    let second = id_at(&s.before, "slide[2]").expect("second slide id");
    assert_eq!(
        s.after
            .find(&second)
            .and_then(|node| node.attrs.get("path"))
            .map(String::as_str),
        Some("slide[3]"),
        "the slide moved and kept its id"
    );
}

#[test]
fn xlsx_ids_survive_an_inserted_row() {
    let bytes = workbook();
    let base = XlsxCodec.project(&bytes).expect("project");
    let row = base
        .nodes()
        .find(|node| node.kind == NodeKind::Row)
        .expect("a row")
        .id
        .to_string();
    let s = step(
        &XlsxCodec,
        &bytes,
        &[insert_after(&row, NodeKind::Row, "new\trow")],
    );
    unchanged_keep_ids(&s);
}

fn workbook() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).expect("sheet");
    sheet.cell_mut("A1").set_value_string("Name");
    sheet.cell_mut("A2").set_value_string("apples");
    sheet.cell_mut("A3").set_value_string("pears");
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut out).expect("write");
    out
}

fn node(id: &str, kind: NodeKind, text: &str, children: Vec<ProjectionNode>) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind,
        text: Some(text.to_owned()),
        hash8: systemprompt_provider_contracts::documents::hash8(text),
        children,
        attrs: Default::default(),
    }
}

fn doc(nodes: Vec<ProjectionNode>) -> Projection {
    Projection {
        format: systemprompt_provider_contracts::documents::DocumentFormat::Markdown,
        root: nodes,
        content_sha256: systemprompt_provider_contracts::documents::content_hash(b""),
    }
}

fn para(id: &str, text: &str) -> ProjectionNode {
    node(id, NodeKind::Paragraph, text, Vec::new())
}

#[test]
fn lcs_carries_ids_past_a_reorder_and_duplicates() {
    let before = doc(vec![
        para("a.1", "one"),
        para("a.2", "two"),
        para("a.3", "same"),
        para("a.4", "same"),
        para("a.5", "three"),
    ]);
    let next = doc(vec![
        para("p0", "two"),
        para("p1", "same"),
        para("p2", "same"),
        para("p3", "one"),
        para("p4", "three"),
    ]);
    let entries = carry_forward(&before, &next, &HashSet::new(), &mut minter("b"));
    let ids: Vec<&str> = entries.iter().map(|e| e.node_id.as_str()).collect();
    assert_eq!(ids[0], "a.2");
    assert_eq!(&ids[1..3], &["a.3", "a.4"]);
    assert_eq!(ids[4], "a.5");
}

#[test]
fn changes_since_report_changed_gaps_and_gone() {
    let since = doc(vec![
        para("a.1", "one"),
        para("a.2", "two"),
        para("a.3", "three"),
        para("a.4", "four"),
    ]);
    let head = doc(vec![
        para("a.1", "one"),
        para("a.2", "TWO"),
        para("a.3", "three"),
        para("b.1", "new"),
    ]);
    let diff = changes(&since, &head, |n| n.text.is_some());
    assert_eq!(diff.changed(), 2);
    assert!(matches!(diff.items[0], ChangeItem::Gap(1)));
    assert!(matches!(diff.items[1], ChangeItem::Node(n) if n.id.as_str() == "a.2"));
    assert!(matches!(diff.items[2], ChangeItem::Gap(1)));
    assert!(matches!(diff.items[3], ChangeItem::Node(n) if n.id.as_str() == "b.1"));
    assert_eq!(diff.gone, vec![DocumentNodeId::new("a.4")]);
}

#[test]
fn derive_ops_expresses_leaf_edits_and_refuses_containers() {
    let body = |children| node("a.0", NodeKind::Body, "", children);
    let base = doc(vec![body(vec![
        para("a.1", "one"),
        para("a.2", "two"),
        para("a.3", "three"),
    ])]);
    let uploaded = doc(vec![body(vec![
        para("a.1", "ONE"),
        para("~upload.1", "inserted"),
        para("a.3", "three"),
    ])]);
    let ops = derive_ops(&base, &uploaded).expect("expressible");
    assert!(ops.iter().any(|op| matches!(op,
        EditOp::ReplaceText { node: NodeTarget::Id(id), text, .. } if id.as_str() == "a.1" && text == "ONE")));
    assert!(ops.iter().any(|op| matches!(op,
        EditOp::InsertAfter { node: NodeTarget::Id(id), text, .. } if id.as_str() == "a.1" && text == "inserted")));
    assert!(ops.iter().any(|op| matches!(op,
        EditOp::Delete { node: NodeTarget::Id(id), .. } if id.as_str() == "a.2")));

    let with_table = doc(vec![body(vec![
        para("a.1", "one"),
        node(
            "~upload.1",
            NodeKind::Table,
            "x",
            vec![para("~upload.2", "x")],
        ),
    ])]);
    assert!(derive_ops(&base, &with_table).is_none());
    assert!(derive_ops(&base, &base).is_none(), "no change is not an op");
}
