use std::collections::BTreeMap;

use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId};
use systemprompt_models::documents::{
    DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash, hash8,
};
use systemprompt_runtime::documents::mcp::{
    MAX_LINES, NodeRange, ReadInput, ReadView, parse_cell, render, split_cell_id,
};

fn node(
    id: &str,
    kind: NodeKind,
    text: Option<&str>,
    children: Vec<ProjectionNode>,
) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind,
        text: text.map(str::to_owned),
        hash8: hash8(text.unwrap_or_default()),
        children,
        attrs: BTreeMap::new(),
    }
}

fn projection(root: Vec<ProjectionNode>) -> Projection {
    Projection {
        format: DocumentFormat::Xlsx,
        root,
        content_sha256: content_hash(b"fixture"),
    }
}

fn input(view: ReadView) -> ReadInput {
    ReadInput {
        document_id: DocumentId::generate(),
        view,
        range: None,
        sheet: None,
        cells: None,
        revision_id: None,
    }
}

fn sheet() -> Projection {
    projection(vec![node(
        "sheet[Sales]",
        NodeKind::Sheet,
        Some("Sales"),
        vec![
            node(
                "sheet[Sales]/A1",
                NodeKind::Cell,
                Some("Region"),
                Vec::new(),
            ),
            node(
                "sheet[Sales]/B7",
                NodeKind::Cell,
                Some("1234.5"),
                Vec::new(),
            ),
        ],
    )])
}

#[test]
fn a1_cells_parse_to_column_and_row() {
    assert_eq!(parse_cell("B7"), Some((2, 7)));
    assert_eq!(parse_cell("aa10"), Some((27, 10)));
    assert_eq!(parse_cell("$C$3"), Some((3, 3)));
    assert_eq!(parse_cell("7B"), None);
    assert_eq!(
        split_cell_id("sheet[Q1 Sales]/B7"),
        Some(("Q1 Sales", "B7"))
    );
    assert_eq!(split_cell_id("body/p[2]"), None);
}

#[test]
fn cells_render_as_sheet_bang_cell_hashlines() {
    let rendered = render(
        &sheet(),
        &input(ReadView::Cells),
        &DocumentRevisionId::generate(),
    )
    .expect("cells");
    assert_eq!(
        rendered.lines,
        vec![
            format!("Sales!A1:{}| Region", hash8("Region")),
            format!("Sales!B7:{}| 1234.5", hash8("1234.5")),
        ]
    );
    let mut narrowed = input(ReadView::Cells);
    narrowed.cells = Some("B1:C9".to_owned());
    narrowed.sheet = Some("Sales".to_owned());
    let rendered = render(&sheet(), &narrowed, &DocumentRevisionId::generate()).expect("cells");
    assert_eq!(
        rendered.lines,
        vec![format!("Sales!B7:{}| 1234.5", hash8("1234.5"))]
    );
}

#[test]
fn bad_cell_range_names_the_input() {
    let mut bad = input(ReadView::Cells);
    bad.cells = Some("not a range".to_owned());
    let error = render(&sheet(), &bad, &DocumentRevisionId::generate()).expect_err("bad range");
    assert!(
        error.message.contains("\"not a range\""),
        "{}",
        error.message
    );
}

#[test]
fn text_view_renders_node_hash_text() {
    let doc = projection(vec![node(
        "body",
        NodeKind::Body,
        None,
        vec![node(
            "body/p[12]",
            NodeKind::Paragraph,
            Some("hello"),
            Vec::new(),
        )],
    )]);
    let rendered = render(
        &doc,
        &input(ReadView::Text),
        &DocumentRevisionId::generate(),
    )
    .expect("text");
    assert_eq!(
        rendered.lines,
        vec![format!("body/p[12]:{}| hello", hash8("hello"))]
    );
    assert!(!rendered.truncated);
}

#[test]
fn range_view_needs_a_range_and_known_nodes() {
    let revision = DocumentRevisionId::generate();
    let error = render(&sheet(), &input(ReadView::Range), &revision).expect_err("no range");
    assert!(
        error.message.contains("range.from_node"),
        "{}",
        error.message
    );
    let mut unknown = input(ReadView::Range);
    unknown.range = Some(NodeRange {
        from_node: DocumentNodeId::new("sheet[Sales]/A1"),
        to_node: DocumentNodeId::new("sheet[Sales]/Z99"),
    });
    let error = render(&sheet(), &unknown, &revision).expect_err("unknown node");
    assert!(
        error.message.contains("sheet[Sales]/Z99"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains(revision.as_str()),
        "{}",
        error.message
    );
}

#[test]
fn long_documents_are_truncated() {
    let lines: Vec<ProjectionNode> = (0..=MAX_LINES)
        .map(|n| node(&format!("line[{n}]"), NodeKind::Text, Some("x"), Vec::new()))
        .collect();
    let rendered = render(
        &projection(lines),
        &input(ReadView::Text),
        &DocumentRevisionId::generate(),
    )
    .expect("text");
    assert_eq!(rendered.lines.len(), MAX_LINES);
    assert!(rendered.truncated);
}

#[test]
fn text_view_carries_headings_lists_cells_and_slides() {
    let doc = projection(vec![
        node(
            "slide[1]",
            NodeKind::Slide,
            Some("Agenda"),
            vec![node(
                "slide[1]/shape[0]/p[0]",
                NodeKind::Heading { level: 2 },
                Some("Plan"),
                Vec::new(),
            )],
        ),
        node(
            "slide[2]",
            NodeKind::Slide,
            None,
            vec![
                node("slide[2]/li", NodeKind::ListItem, Some("one"), Vec::new()),
                node("slide[2]/tc", NodeKind::Cell, Some("a"), Vec::new()),
            ],
        ),
    ]);
    let rendered = render(
        &doc,
        &input(ReadView::Text),
        &DocumentRevisionId::generate(),
    )
    .expect("text");
    let shown: Vec<&str> = rendered
        .lines
        .iter()
        .map(|line| line.split_once("| ").expect("hashline").1)
        .collect();
    assert_eq!(
        shown,
        vec!["## slide 1: Agenda", "## Plan", "## slide 2", "- one", "| a"]
    );
    let outline = render(
        &doc,
        &input(ReadView::Outline),
        &DocumentRevisionId::generate(),
    )
    .expect("outline");
    assert_eq!(outline.lines[1], rendered.lines[1]);
}
