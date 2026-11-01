use std::collections::BTreeMap;

use systemprompt_files::documents::nodes::PATH_ATTR;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{
    DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash, hash8,
};
use systemprompt_provider_contracts::documents::READ_ONLY_ATTR;
use systemprompt_runtime::documents::views::{MAX_WRITTEN, shown_written_lines, touched_lines};

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

fn para(id: &str, text: &str) -> ProjectionNode {
    node(id, NodeKind::Paragraph, Some(text), Vec::new())
}

fn with_attr(mut node: ProjectionNode, key: &str, value: &str) -> ProjectionNode {
    node.attrs.insert(key.to_owned(), value.to_owned());
    node
}

fn projection(root: Vec<ProjectionNode>) -> Projection {
    Projection {
        format: DocumentFormat::Markdown,
        root,
        content_sha256: content_hash(b"fixture"),
    }
}

fn line(id: &str, text: &str) -> String {
    format!("{id}:{}| {text}", hash8(text))
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

fn markdown(outline_text: &str, body_text: &str) -> Projection {
    projection(vec![
        with_attr(
            node(
                "outline",
                NodeKind::Body,
                None,
                vec![para("o1", outline_text)],
            ),
            READ_ONLY_ATTR,
            "true",
        ),
        node("body", NodeKind::Body, None, vec![para("b1", body_text)]),
    ])
}

#[test]
fn named_nodes_render_by_id_or_by_path() {
    let doc = projection(vec![
        para("n1", "first"),
        with_attr(para("n2", "second"), PATH_ATTR, "/body/2"),
        para("n3", "third"),
        node("empty", NodeKind::Body, None, Vec::new()),
    ]);

    let lines = touched_lines(&doc, &ids(&["n1", "/body/2", "empty"]), &[], false);

    assert_eq!(lines, vec![line("n1", "first"), line("n2", "second")]);
}

#[test]
fn a_folded_cell_shows_when_its_paragraph_was_written() {
    let doc = projection(vec![node(
        "sheet",
        NodeKind::Sheet,
        Some("Sales"),
        vec![node(
            "sheet/A1",
            NodeKind::Cell,
            Some("42"),
            vec![para("sheet/A1/p", "42")],
        )],
    )]);

    let lines = touched_lines(&doc, &ids(&["sheet/A1/p"]), &[], false);

    assert_eq!(lines, vec![format!("sheet/A1:{}| | 42", hash8("42"))]);
}

#[test]
fn a_write_under_a_hidden_root_shows_the_visible_mirror_line() {
    let doc = markdown("Hello", "Hello");

    let hidden = touched_lines(&doc, &ids(&["o1"]), &[], false);
    assert_eq!(hidden, vec![line("b1", "Hello")]);

    let verbose = touched_lines(&doc, &ids(&["o1"]), &[], true);
    assert_eq!(verbose, vec![line("o1", "Hello")]);
}

#[test]
fn a_hidden_write_without_a_visible_twin_shows_nothing() {
    let doc = markdown("Only in outline", "Body text");

    assert!(touched_lines(&doc, &ids(&["o1"]), &[], false).is_empty());
}

#[test]
fn written_values_outside_the_projection_render_from_the_input() {
    let doc = projection(vec![para("n1", "kept")]);
    let values = vec![
        ("n1".to_owned(), "ignored".to_owned()),
        ("sheet[S]/Z900".to_owned(), "7".to_owned()),
    ];

    let lines = touched_lines(&doc, &ids(&["n1"]), &values, false);

    assert_eq!(
        lines,
        vec![
            line("n1", "kept"),
            format!("sheet[S]/Z900:{}| | 7", hash8("7")),
        ]
    );
}

#[test]
fn touched_lines_cap_at_max_written() {
    let root: Vec<ProjectionNode> = (0..MAX_WRITTEN + 20)
        .map(|i| para(&format!("n{i}"), &format!("text {i}")))
        .collect();
    let touched: Vec<String> = (0..MAX_WRITTEN + 20).map(|i| format!("n{i}")).collect();

    let lines = touched_lines(&projection(root), &touched, &[], false);

    assert_eq!(lines.len(), MAX_WRITTEN);
    assert_eq!(lines[0], line("n0", "text 0"));
}

#[test]
fn shown_written_lines_hide_read_only_roots_unless_verbose() {
    let base = markdown("Old", "Old");
    let head = markdown("New outline", "New body");

    assert_eq!(
        shown_written_lines(&base, &head, false),
        vec![line("b1", "New body")]
    );
    assert_eq!(
        shown_written_lines(&base, &head, true),
        vec![line("o1", "New outline"), line("b1", "New body")]
    );
}
