//! Hashline rendering of a projection for the read views.
//!
//! A line is `<node id>:<hash8>| <text>`; a spreadsheet cell renders as
//! `<sheet>!<cell>:<hash8>| <value>`. The hash is the anchor an edit quotes
//! back as `expected_hash`. The text carries the node's structure the way
//! the outline does: `# `…`###### ` by heading level, `- ` for a list item,
//! `| ` for a table cell and `## slide N` at each slide boundary. The
//! prefix is presentation only; the hash covers the node's text alone. Node
//! ids are stable across revisions; a cell's sheet and A1 address come from
//! its codec path.
//!
//! The text views skip a read-only root (a Markdown document's parsed
//! `outline`, which repeats its `body` lines as blocks) unless the read is
//! `verbose`; the projection itself keeps both.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_files::documents::DocumentsError;
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};


use systemprompt_files::documents::nodes::path_of;

use super::outline::{node_lines, outline, search_lines, shown_roots};
use super::types::{NodeRange, ReadInput, ReadView};

pub const MAX_LINES: usize = 2000;
pub const MAX_WRITTEN: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLines {
    pub lines: Vec<String>,
    pub truncated: bool,
}

pub fn render(projection: &Projection, input: &ReadInput) -> Result<RenderedLines, DocumentsError> {
    let lines = match input.view {
        ReadView::Text | ReadView::Changes | ReadView::Meta | ReadView::Model => {
            let shown = shown_roots(projection, input.verbose);
            structured(shown.as_ref().unwrap_or(projection), shows)
        },
        ReadView::Outline => outline(projection),
        ReadView::Node => node_lines(projection, input.node_id.as_ref())?,
        ReadView::Search => search_lines(projection, input.text.as_deref())?,
        ReadView::Range => {
            let range = input.range.as_ref().ok_or(DocumentsError::InvalidParams {
                detail: "view range needs range.from_node and range.to_node; read the outline \
                         for node ids",
            })?;
            range_lines(projection, range)?
        },
        ReadView::Cells => cell_lines(projection, input.sheet.as_deref(), input.cells.as_deref())?,
    };
    Ok(truncate(lines))
}

#[must_use]
pub fn structured_line(node: &ProjectionNode, slide: usize) -> String {
    let text = node.text.as_deref().unwrap_or_default();
    let shown = match node.kind {
        NodeKind::Heading { level } => {
            format!("{} {text}", "#".repeat(usize::from(level.clamp(1, 6))))
        },
        NodeKind::ListItem => format!("- {text}"),
        NodeKind::Pending => format!("[pending] {text}"),
        NodeKind::Cell => format!("| {text}"),
        NodeKind::Slide if text.is_empty() => format!("## slide {slide}"),
        NodeKind::Slide => format!("## slide {slide}: {text}"),
        _ => text.to_owned(),
    };
    format!("{}:{}| {shown}", node.id.as_str(), node.hash8)
}

pub(crate) fn structured(
    projection: &Projection,
    mut keep: impl FnMut(&ProjectionNode) -> bool,
) -> Vec<String> {
    let mut slide = 0;
    let mut lines = Vec::new();
    for node in &projection.root {
        walk(node, &mut slide, &mut keep, &mut lines);
    }
    lines
}

pub(super) fn folded_cell(node: &ProjectionNode) -> bool {
    node.kind == NodeKind::Cell
        && node.children.len() == 1
        && node.children[0].kind == NodeKind::Paragraph
        && node.children[0].text == node.text
}

fn walk(
    node: &ProjectionNode,
    slide: &mut usize,
    keep: &mut impl FnMut(&ProjectionNode) -> bool,
    lines: &mut Vec<String>,
) {
    if node.kind == NodeKind::Slide {
        *slide += 1;
    }
    if keep(node) {
        lines.push(structured_line(node, *slide));
    }
    if folded_cell(node) {
        return;
    }
    for child in &node.children {
        walk(child, slide, keep, lines);
    }
}

pub(crate) fn shows(node: &ProjectionNode) -> bool {
    node.text.is_some() || node.kind == NodeKind::Slide
}

#[must_use]
pub fn written_lines(base: Option<&Projection>, head: &Projection) -> Vec<String> {
    let mut unchanged: HashMap<&str, usize> = HashMap::new();
    for node in base
        .into_iter()
        .flat_map(Projection::nodes)
        .filter(|n| shows(n))
    {
        *unchanged.entry(node.hash8.as_str()).or_default() += 1;
    }
    let mut lines = structured(head, |node| {
        if !shows(node) {
            return false;
        }
        match unchanged.get_mut(node.hash8.as_str()) {
            Some(left) if *left > 0 => {
                *left -= 1;
                false
            },
            _ => true,
        }
    });
    lines.truncate(MAX_WRITTEN);
    lines
}

pub(super) const fn is_outline(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Heading { .. }
            | NodeKind::Slide
            | NodeKind::Sheet
            | NodeKind::Table
            | NodeKind::Pending
    )
}

pub(crate) fn truncate(mut lines: Vec<String>) -> RenderedLines {
    let truncated = lines.len() > MAX_LINES;
    lines.truncate(MAX_LINES);
    RenderedLines { lines, truncated }
}

fn range_lines(projection: &Projection, range: &NodeRange) -> Result<Vec<String>, DocumentsError> {
    for node in [&range.from_node, &range.to_node] {
        if projection.find(node).is_none() {
            return Err(DocumentsError::UnknownNode { node: node.clone() });
        }
    }
    let mut inside = false;
    let mut done = false;
    Ok(structured(projection, |node| {
        inside |= node.id == range.from_node;
        let keep = inside && !done;
        done |= inside && node.id == range.to_node;
        keep
    }))
}

fn cell_lines(
    projection: &Projection,
    sheet: Option<&str>,
    cells: Option<&str>,
) -> Result<Vec<String>, DocumentsError> {
    let bounds = cells.map(parse_range).transpose()?;
    Ok(projection
        .nodes()
        .filter(|node| node.kind == NodeKind::Cell)
        .filter_map(|node| {
            let (name, cell) = split_cell_id(path_of(node))?;
            if sheet.is_some_and(|wanted| wanted != name) {
                return None;
            }
            if let Some(bounds) = bounds
                && !within(parse_cell(cell)?, bounds)
            {
                return None;
            }
            Some(format!(
                "{name}!{cell}:{}| {}",
                node.hash8,
                node.text.as_deref().unwrap_or_default()
            ))
        })
        .collect())
}

pub fn split_cell_id(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix("sheet[")?.rsplit_once("]/")
}

type Cell = (u32, u32);

fn parse_range(range: &str) -> Result<(Cell, Cell), DocumentsError> {
    let invalid = || DocumentsError::InvalidParams {
        detail: "cells is not an A1 cell or range such as B7 or A1:C9",
    };
    let (start, end) = range.split_once(':').unwrap_or((range, range));
    let start = parse_cell(start).ok_or_else(invalid)?;
    let end = parse_cell(end).ok_or_else(invalid)?;
    Ok((
        (start.0.min(end.0), start.1.min(end.1)),
        (start.0.max(end.0), start.1.max(end.1)),
    ))
}

pub fn parse_cell(cell: &str) -> Option<Cell> {
    let cell = cell.replace('$', "");
    let split = cell.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = cell.split_at(split);
    if letters.is_empty() || !letters.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let column = letters
        .to_ascii_uppercase()
        .bytes()
        .try_fold(0u32, |acc, b| {
            acc.checked_mul(26)?.checked_add(u32::from(b - b'A') + 1)
        })?;
    let row = digits.parse::<u32>().ok()?;
    Some((column, row))
}

const fn within(cell: Cell, (start, end): (Cell, Cell)) -> bool {
    cell.0 >= start.0 && cell.0 <= end.0 && cell.1 >= start.1 && cell.1 <= end.1
}
