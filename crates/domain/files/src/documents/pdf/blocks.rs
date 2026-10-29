//! The flat block list a projection is rendered from: every node kind of
//! every codec lowers to a heading, a paragraph, a list item, a table, a
//! section title or a page break, so one layout serves every format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_provider_contracts::documents::{NodeKind, Projection, ProjectionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    Body,
    Italic,
    Mono,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Block {
    Heading { level: u8, text: String },
    Paragraph { style: Style, text: String },
    ListItem { depth: usize, text: String },
    Table { rows: Vec<Vec<String>> },
    PageBreak,
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
        NodeKind::Heading { level } => out.push(Block::Heading {
            level,
            text: text_of(node),
        }),
        NodeKind::Paragraph if node.attrs.get("bullet").is_some_and(|v| v == "true") => {
            out.push(Block::ListItem {
                depth: list_depth,
                text: text_of(node),
            });
        },
        NodeKind::Paragraph => out.push(Block::Paragraph {
            style: Style::Body,
            text: text_of(node),
        }),
        NodeKind::Text if !node.children.is_empty() => {
            for child in &node.children {
                walk(child, list_depth, out);
            }
        },
        NodeKind::Text => out.push(Block::Paragraph {
            style: Style::Mono,
            text: node.text.clone().unwrap_or_default(),
        }),
        NodeKind::Pending => out.push(Block::Paragraph {
            style: Style::Italic,
            text: format!("[pending: {}]", node.text.as_deref().unwrap_or_default()),
        }),
        NodeKind::ListItem => {
            out.push(Block::ListItem {
                depth: list_depth,
                text: own_text(node),
            });
            for child in &node.children {
                walk(child, list_depth + 1, out);
            }
        },
        NodeKind::Table => out.push(Block::Table {
            rows: table_rows(node),
        }),
        NodeKind::Sheet => {
            out.push(Block::Heading {
                level: 2,
                text: node.text.clone().unwrap_or_default(),
            });
            out.push(Block::Table {
                rows: sheet_rows(node),
            });
        },
        NodeKind::Slide => {
            out.push(Block::PageBreak);
            let title = node.text.clone().unwrap_or_default();
            if !title.is_empty() {
                out.push(Block::Heading {
                    level: 1,
                    text: title,
                });
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

fn text_of(node: &ProjectionNode) -> String {
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

fn rows_in(node: &ProjectionNode, rows: &mut Vec<ProjectionNode>) {
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

fn column_index(id: &str) -> Option<usize> {
    let reference = id.rsplit('/').next()?;
    let letters: String = reference
        .chars()
        .take_while(char::is_ascii_uppercase)
        .collect();
    if letters.is_empty()
        || !reference[letters.len()..]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let number = letters.chars().try_fold(0u32, |acc, c| {
        acc.checked_mul(26)?
            .checked_add(u32::from(c) - u32::from('A') + 1)
    })?;
    usize::try_from(number - 1).ok()
}

fn column_label(index: usize) -> String {
    let mut n = index + 1;
    let mut label = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        label.insert(0, char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        n = (n - 1) / 26;
    }
    label
}

fn sheet_rows(sheet: &ProjectionNode) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    rows_in(sheet, &mut rows);
    let placed: Vec<(String, BTreeMap<usize, String>, String)> = rows
        .iter()
        .map(|row| {
            let number = row
                .id
                .as_str()
                .rsplit("row[")
                .next()
                .map(|rest| rest.trim_end_matches(']').to_owned())
                .unwrap_or_default();
            let cells: BTreeMap<usize, String> = row
                .children
                .iter()
                .filter_map(|cell| Some((column_index(cell.id.as_str())?, text_of(cell))))
                .collect();
            (number, cells, text_of(row))
        })
        .collect();
    let first = placed
        .iter()
        .filter_map(|(_, cells, _)| cells.keys().next().copied())
        .min()
        .unwrap_or(0);
    let last = placed
        .iter()
        .filter_map(|(_, cells, _)| cells.keys().next_back().copied())
        .max()
        .unwrap_or(0);
    let mut grid = Vec::with_capacity(placed.len() + 1);
    let mut header = vec![String::new()];
    header.extend((first..=last).map(column_label));
    grid.push(header);
    for (number, cells, fallback) in placed {
        let mut line = vec![number];
        if cells.is_empty() {
            line.push(fallback);
            line.resize(last - first + 2, String::new());
        } else {
            line.extend((first..=last).map(|col| cells.get(&col).cloned().unwrap_or_default()));
        }
        grid.push(line);
    }
    grid
}
