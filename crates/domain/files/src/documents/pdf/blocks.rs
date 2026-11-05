//! The flat block list a projection is rendered from: every node kind of
//! every codec lowers to a heading, a paragraph, a list item, a table, a
//! section title or a page break, so one layout serves every format. Formats
//! with a rich text model reach the same blocks through `rich`, with their
//! styling; these blocks are plain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use systemprompt_provider_contracts::documents::{NodeKind, Projection, ProjectionNode};

use super::layout_model::Align;
use super::runs::{Rgb, Run};
use super::sheet_rows::sheet_rows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    Body,
    Quote,
    Callout,
    Code,
    Lines,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Cell {
    pub(super) runs: Vec<Run>,
    pub(super) shade: Option<Rgb>,
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Self {
            runs: vec![Run::plain(text)],
            shade: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Block {
    Heading {
        level: u8,
        runs: Vec<Run>,
        align: Align,
    },
    Paragraph {
        style: Style,
        runs: Vec<Run>,
        align: Align,
    },
    ListItem {
        depth: usize,
        marker: String,
        runs: Vec<Run>,
    },
    Table {
        rows: Vec<Vec<Cell>>,
        header_rows: usize,
    },
    Image {
        picture: usize,
        width: u32,
        height: u32,
        align: Align,
    },
    Divider,
    PageBreak,
}

pub(super) const BULLET: &str = "\u{2022}";

fn heading(level: u8, text: String) -> Block {
    Block::Heading {
        level,
        runs: vec![Run::plain(text)],
        align: Align::Left,
    }
}

fn paragraph(style: Style, run: Run) -> Block {
    Block::Paragraph {
        style,
        runs: vec![run],
        align: Align::Left,
    }
}

fn item(depth: usize, text: String) -> Block {
    Block::ListItem {
        depth,
        marker: BULLET.to_owned(),
        runs: vec![Run::plain(text)],
    }
}

fn table(rows: Vec<Vec<String>>, header_rows: usize) -> Block {
    Block::Table {
        rows: rows
            .into_iter()
            .map(|row| row.into_iter().map(Cell::from).collect())
            .collect(),
        header_rows,
    }
}

#[must_use]
pub(super) fn blocks_of(projection: &Projection) -> Vec<Block> {
    let mut out = Vec::new();
    let outline: Vec<&ProjectionNode> = projection
        .root
        .iter()
        .filter(|node| node.kind == NodeKind::Markdown)
        .collect();
    let roots: Vec<&ProjectionNode> = if outline.is_empty() {
        projection.root.iter().collect()
    } else {
        outline
    };
    for node in roots {
        walk(node, 0, &mut out);
    }
    out
}

fn walk(node: &ProjectionNode, list_depth: usize, out: &mut Vec<Block>) {
    match node.kind {
        NodeKind::Heading { level } => out.push(heading(level, text_of(node))),
        NodeKind::Paragraph if node.attrs.get("bullet").is_some_and(|v| v == "true") => {
            out.push(item(list_depth, text_of(node)));
        },
        NodeKind::Paragraph => out.push(paragraph(Style::Body, Run::plain(text_of(node)))),
        NodeKind::Text if !node.children.is_empty() => {
            for child in &node.children {
                walk(child, list_depth, out);
            }
        },
        NodeKind::Text => out.push(paragraph(
            Style::Lines,
            Run {
                mono: true,
                ..Run::plain(node.text.clone().unwrap_or_default())
            },
        )),
        NodeKind::Pending => out.push(paragraph(
            Style::Body,
            Run {
                italic: true,
                ..Run::plain(format!(
                    "[pending: {}]",
                    node.text.as_deref().unwrap_or_default()
                ))
            },
        )),
        NodeKind::ListItem => {
            out.push(item(list_depth, own_text(node)));
            for child in &node.children {
                walk(child, list_depth + 1, out);
            }
        },
        NodeKind::Table => out.push(table(table_rows(node), 0)),
        NodeKind::Sheet => {
            out.push(heading(2, node.text.clone().unwrap_or_default()));
            out.push(table(sheet_rows(node), 1));
        },
        NodeKind::Slide => {
            out.push(Block::PageBreak);
            let title = node.text.clone().unwrap_or_default();
            if !title.is_empty() {
                out.push(heading(1, title));
            }
            for child in &node.children {
                walk(child, list_depth, out);
            }
        },
        NodeKind::Image => {},
        NodeKind::Body
        | NodeKind::Row
        | NodeKind::Cell
        | NodeKind::Range
        | NodeKind::Shape
        | NodeKind::Html
        | NodeKind::Markdown => {
            for child in &node.children {
                walk(child, list_depth, out);
            }
        },
    }
}

fn own_text(node: &ProjectionNode) -> String {
    node.text
        .as_deref()
        .map(normalise)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| {
            node.children
                .iter()
                .filter(|child| child.kind != NodeKind::ListItem)
                .map(text_of)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
}

pub(super) fn text_of(node: &ProjectionNode) -> String {
    if let Some(text) = node
        .text
        .as_deref()
        .map(normalise)
        .filter(|t| !t.is_empty())
    {
        return text;
    }
    node.children
        .iter()
        .map(text_of)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn rows_in(node: &ProjectionNode, rows: &mut Vec<ProjectionNode>) {
    for child in &node.children {
        if child.kind == NodeKind::Row {
            rows.push(child.clone());
        } else {
            rows_in(child, rows);
        }
    }
}

fn cells_in(node: &ProjectionNode, cells: &mut Vec<String>) {
    for child in &node.children {
        if child.kind == NodeKind::Cell || child.kind == NodeKind::Pending {
            cells.push(text_of(child));
        } else {
            cells_in(child, cells);
        }
    }
}

fn table_rows(table: &ProjectionNode) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    rows_in(table, &mut rows);
    rows.iter()
        .map(|row| {
            let mut cells = Vec::new();
            cells_in(row, &mut cells);
            if cells.is_empty() {
                cells.push(text_of(row));
            }
            cells
        })
        .collect()
}
