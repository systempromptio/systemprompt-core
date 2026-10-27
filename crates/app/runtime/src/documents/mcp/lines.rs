//! Hashline rendering of a projection for `documents_read`.
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
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};

use systemprompt_files::documents::nodes::path_of;

use super::inputs::{NodeRange, ReadInput, ReadView};
use super::refusal::{Refusal, RefusalCode};
use super::views::{node_lines, outline, search_lines};

pub const MAX_LINES: usize = 2000;
pub const MAX_WRITTEN: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLines {
    pub lines: Vec<String>,
    pub truncated: bool,
}

pub fn render(
    projection: &Projection,
    input: &ReadInput,
    revision: &DocumentRevisionId,
) -> Result<RenderedLines, McpError> {
    let lines = match input.view {
        ReadView::Text | ReadView::Changes | ReadView::Meta => structured(projection, shows),
        ReadView::Outline => outline(projection),
        ReadView::Node => node_lines(projection, input.node_id.as_ref(), revision)?,
        ReadView::Search => search_lines(projection, input.text.as_deref())?,
        ReadView::Range => {
            let range = input.range.as_ref().ok_or_else(|| {
                McpError::invalid_params(
                    "view range needs range.from_node and range.to_node; read the outline for \
                     node ids",
                    None,
                )
            })?;
            range_lines(projection, range, revision)?
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

pub(super) fn structured(
    projection: &Projection,
    mut keep: impl FnMut(&ProjectionNode) -> bool,
) -> Vec<String> {
    let mut slide = 0;
    projection
        .nodes()
        .filter_map(|node| {
            if node.kind == NodeKind::Slide {
                slide += 1;
            }
            keep(node).then(|| structured_line(node, slide))
        })
        .collect()
}

pub(super) fn shows(node: &ProjectionNode) -> bool {
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

pub(super) fn truncate(mut lines: Vec<String>) -> RenderedLines {
    let truncated = lines.len() > MAX_LINES;
    lines.truncate(MAX_LINES);
    RenderedLines { lines, truncated }
}

fn range_lines(
    projection: &Projection,
    range: &NodeRange,
    revision: &DocumentRevisionId,
) -> Result<Vec<String>, McpError> {
    for node in [&range.from_node, &range.to_node] {
        if projection.find(node).is_none() {
            return Err(unknown_node(node, revision));
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

pub(super) fn unknown_node(node: &DocumentNodeId, revision: &DocumentRevisionId) -> McpError {
    Refusal {
        node: Some(node.clone()),
        ..Refusal::new(RefusalCode::NotFound)
    }
    .into_error(format!(
        "node {node} does not exist in revision {revision}; read the outline again for current \
         node ids"
    ))
}

fn cell_lines(
    projection: &Projection,
    sheet: Option<&str>,
    cells: Option<&str>,
) -> Result<Vec<String>, McpError> {
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

fn parse_range(range: &str) -> Result<(Cell, Cell), McpError> {
    let invalid = || {
        McpError::invalid_params(
            format!("cells \"{range}\" is not an A1 cell or range such as B7 or A1:C9"),
            None,
        )
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
