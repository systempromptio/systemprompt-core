use std::collections::BTreeMap;

use systemprompt_files::documents::DocumentsError;
use systemprompt_identifiers::{DocumentId, DocumentNodeId};
use systemprompt_models::documents::{
    DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash, hash8,
};
use systemprompt_runtime::documents::views::{
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
        since_revision: None,
        node_id: None,
        text: None,
        verbose: false,
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
    let rendered = render(&sheet(), &input(ReadView::Cells)).expect("cells");
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
    let rendered = render(&sheet(), &narrowed).expect("cells");
    assert_eq!(
        rendered.lines,
        vec![format!("Sales!B7:{}| 1234.5", hash8("1234.5"))]
    );
}

#[test]
fn bad_cell_range_names_the_input() {
    let mut bad = input(ReadView::Cells);
    bad.cells = Some("not a range".to_owned());
    let error = render(&sheet(), &bad).expect_err("bad range");
    assert!(
        matches!(error, DocumentsError::InvalidParams { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("A1 cell or range"), "{error}");
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
    let rendered = render(&doc, &input(ReadView::Text)).expect("text");
    assert_eq!(
        rendered.lines,
        vec![format!("body/p[12]:{}| hello", hash8("hello"))]
    );
    assert!(!rendered.truncated);
}

#[test]
fn range_view_needs_a_range_and_known_nodes() {
    let error = render(&sheet(), &input(ReadView::Range)).expect_err("no range");
    assert!(error.to_string().contains("range.from_node"), "{error}");
    let mut unknown = input(ReadView::Range);
    unknown.range = Some(NodeRange {
        from_node: DocumentNodeId::new("sheet[Sales]/A1"),
        to_node: DocumentNodeId::new("sheet[Sales]/Z99"),
    });
    let error = render(&sheet(), &unknown).expect_err("unknown node");
    assert!(
        matches!(&error, DocumentsError::UnknownNode { node } if node.as_str() == "sheet[Sales]/Z99"),
        "{error}"
    );
}

#[test]
fn long_documents_are_truncated() {
    let lines: Vec<ProjectionNode> = (0..=MAX_LINES)
        .map(|n| node(&format!("line[{n}]"), NodeKind::Text, Some("x"), Vec::new()))
        .collect();
    let rendered = render(&projection(lines), &input(ReadView::Text)).expect("text");
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
    let rendered = render(&doc, &input(ReadView::Text)).expect("text");
    let shown: Vec<&str> = rendered
        .lines
        .iter()
        .map(|line| line.split_once("| ").expect("hashline").1)
        .collect();
    assert_eq!(
        shown,
        vec![
            "## slide 1: Agenda",
            "## Plan",
            "## slide 2",
            "- one",
            "| a"
        ]
    );
    let outline = render(&doc, &input(ReadView::Outline)).expect("outline");
    assert_eq!(outline.lines[1], rendered.lines[1]);
}

#[test]
fn written_lines_are_the_nodes_a_revision_added_or_changed() {
    let para = |id: &str, text: &str| node(id, NodeKind::Paragraph, Some(text), Vec::new());
    let base = projection(vec![para("p[0]", "keep"), para("p[1]", "old")]);
    let head = projection(vec![
        para("p[0]", "keep"),
        para("p[1]", "new"),
        para("p[2]", "old"),
        node(
            "p[3]",
            NodeKind::Heading { level: 1 },
            Some("Title"),
            Vec::new(),
        ),
    ]);
    assert_eq!(
        systemprompt_runtime::documents::views::written_lines(Some(&base), &head),
        vec![
            format!("p[1]:{}| new", hash8("new")),
            format!("p[3]:{}| # Title", hash8("Title")),
        ]
    );
    assert_eq!(
        systemprompt_runtime::documents::views::written_lines(None, &base).len(),
        2
    );
}

fn markdown() -> Projection {
    use systemprompt_files::documents::codecs::MarkdownCodec;
    use systemprompt_provider_contracts::documents::DocumentCodec;
    MarkdownCodec
        .project(b"# Plan\n\nShip it.\n")
        .expect("project")
}

#[test]
fn a_markdown_text_read_lists_each_block_once() {
    let text = render(&markdown(), &input(ReadView::Text)).expect("text");
    assert!(
        text.lines.iter().all(|line| line.starts_with("line[")),
        "{:?}",
        text.lines
    );
    assert_eq!(
        text.lines
            .iter()
            .filter(|line| line.ends_with("Ship it."))
            .count(),
        1,
        "{:?}",
        text.lines
    );
}

#[test]
fn a_verbose_markdown_read_shows_the_read_only_outline() {
    let mut verbose = input(ReadView::Text);
    verbose.verbose = true;
    let text = render(&markdown(), &verbose).expect("text");
    assert!(text.lines.iter().any(|line| line.starts_with("h[")));
}
