use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use systemprompt_files::documents::pdf::{RenderRequest, render};
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_provider_contracts::documents::{
    DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash,
};
use systemprompt_test_fixtures::parse_pdf;

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
        hash8: "00000000".to_owned(),
        children,
        attrs: BTreeMap::new(),
    }
}

fn leaf(id: &str, kind: NodeKind, text: &str) -> ProjectionNode {
    node(id, kind, Some(text), Vec::new())
}

fn text_of(root: Vec<ProjectionNode>) -> String {
    let projection = Projection {
        format: DocumentFormat::Markdown,
        root,
        content_sha256: content_hash(b"hand"),
    };
    let revision = DocumentRevisionId::new("drev_01HZZZZZZZZZZZZZZZZZZZZZZZ");
    let pdf = render(
        &projection,
        &RenderRequest {
            name: "hand.md",
            revision: &revision,
            created_at: Utc
                .with_ymd_and_hms(2026, 10, 31, 9, 0, 0)
                .single()
                .expect("date"),
            max_pages: 50,
        },
    )
    .expect("render");
    parse_pdf(&pdf).text()
}

fn assert_carries(text: &str, needles: &[&str]) {
    for needle in needles {
        assert!(text.contains(needle), "{needle:?} missing from:\n{text}");
    }
}

#[test]
fn a_markdown_outline_wins_over_sibling_roots_and_pending_and_bullets_render() {
    let mut bullet = leaf("b", NodeKind::Paragraph, "bulleted   line");
    bullet.attrs.insert("bullet".to_owned(), "true".to_owned());
    let outline = node(
        "md",
        NodeKind::Markdown,
        None,
        vec![
            leaf("h", NodeKind::Heading { level: 3 }, "Outline heading"),
            bullet,
            leaf("p", NodeKind::Pending, "awaiting review"),
            node(
                "t",
                NodeKind::Text,
                None,
                vec![leaf("t1", NodeKind::Paragraph, "nested text child")],
            ),
            leaf("code", NodeKind::Text, "raw mono"),
            leaf("img", NodeKind::Image, "never shown"),
        ],
    );
    let ignored = leaf("x", NodeKind::Paragraph, "outside the outline");
    let text = text_of(vec![ignored, outline]);
    assert_carries(
        &text,
        &[
            "Outline heading",
            "bulleted line",
            "[pending: awaiting review]",
            "nested text child",
            "raw mono",
        ],
    );
    assert!(!text.contains("outside the outline"));
    assert!(!text.contains("never shown"));
}

#[test]
fn list_items_take_their_own_text_or_their_non_list_children() {
    let inner = leaf("li2", NodeKind::ListItem, "deeper item");
    let worded = node(
        "li1",
        NodeKind::ListItem,
        None,
        vec![leaf("p1", NodeKind::Paragraph, "from child"), inner],
    );
    let titled = node(
        "li3",
        NodeKind::ListItem,
        Some("  own   words "),
        vec![leaf("p2", NodeKind::Paragraph, "skipped child")],
    );
    let text = text_of(vec![node(
        "body",
        NodeKind::Body,
        None,
        vec![worded, titled],
    )]);
    assert_carries(
        &text,
        &["from child", "deeper item", "own words", "skipped child"],
    );
}

#[test]
fn tables_read_cells_or_fall_back_to_the_row_text() {
    let table = node(
        "tbl",
        NodeKind::Table,
        None,
        vec![
            node(
                "r1",
                NodeKind::Row,
                None,
                vec![
                    leaf("c1", NodeKind::Cell, "alpha"),
                    leaf("c2", NodeKind::Pending, "beta"),
                ],
            ),
            node("r2", NodeKind::Row, Some("whole row"), Vec::new()),
        ],
    );
    let text = text_of(vec![table]);
    assert_carries(&text, &["alpha", "beta", "whole row"]);
}

#[test]
fn sheets_grid_their_cells_by_column_letter_and_slides_break_pages() {
    let sheet = node(
        "sheet[Data]",
        NodeKind::Sheet,
        Some("Data"),
        vec![
            node(
                "sheet[Data]/row[2]",
                NodeKind::Row,
                None,
                vec![
                    leaf("sheet[Data]/AA2", NodeKind::Cell, "far right"),
                    leaf("sheet[Data]/C2", NodeKind::Cell, "third"),
                    leaf("sheet[Data]/bad", NodeKind::Cell, "unplaced"),
                ],
            ),
            node(
                "sheet[Data]/row[9]",
                NodeKind::Row,
                Some("loose row"),
                Vec::new(),
            ),
        ],
    );
    let slide = node(
        "slide[1]",
        NodeKind::Slide,
        Some("Slide title"),
        vec![node(
            "slide[1]/shape[1]",
            NodeKind::Shape,
            None,
            vec![leaf(
                "slide[1]/shape[1]/p[1]",
                NodeKind::Paragraph,
                "slide body",
            )],
        )],
    );
    let untitled = node(
        "slide[2]",
        NodeKind::Slide,
        None,
        vec![leaf("slide[2]/p", NodeKind::Paragraph, "second slide")],
    );
    let text = text_of(vec![sheet, slide, untitled]);
    assert_carries(
        &text,
        &[
            "Data",
            "AA",
            "C",
            "far right",
            "third",
            "loose\nrow",
            "Slide title",
            "slide body",
            "second slide",
        ],
    );
    assert!(!text.contains("unplaced"));
}
