//! Hashline rendering of a projection for `documents_read`.
//!
//! A line is `<node id>:<hash8>| <text>`; a spreadsheet cell renders as
//! `<sheet>!<cell>:<hash8>| <value>`. The hash is the anchor an edit quotes
//! back as `expected_hash`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};
use systemprompt_models::documents::{NodeKind, Projection, ProjectionNode};

use super::inputs::{NodeRange, ReadInput, ReadView};

pub const MAX_LINES: usize = 2000;

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
        ReadView::Text => projection
            .nodes()
            .filter(|node| node.text.is_some())
            .map(ProjectionNode::hashline)
            .collect(),
        ReadView::Outline => projection
            .nodes()
            .filter(|node| is_outline(node.kind))
            .map(ProjectionNode::hashline)
            .collect(),
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

const fn is_outline(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Heading { .. } | NodeKind::Slide | NodeKind::Sheet | NodeKind::Table
    )
}

fn truncate(mut lines: Vec<String>) -> RenderedLines {
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
    Ok(projection.lines(Some((&range.from_node, &range.to_node))))
}

fn unknown_node(node: &DocumentNodeId, revision: &DocumentRevisionId) -> McpError {
    McpError::invalid_params(
        format!(
            "node {node} does not exist in revision {revision}; read the outline again for \
             current node ids"
        ),
        None,
    )
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
            let (name, cell) = split_cell_id(node.id.as_str())?;
            if sheet.is_some_and(|wanted| wanted != name) {
                return None;
            }
            if let Some(bounds) = bounds {
                if !within(parse_cell(cell)?, bounds) {
                    return None;
                }
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
        .try_fold(0_u32, |acc, b| {
            acc.checked_mul(26)?.checked_add(u32::from(b - b'A') + 1)
        })?;
    let row = digits.parse::<u32>().ok()?;
    Some((column, row))
}

const fn within(cell: Cell, (start, end): (Cell, Cell)) -> bool {
    cell.0 >= start.0 && cell.0 <= end.0 && cell.1 >= start.1 && cell.1 <= end.1
}
