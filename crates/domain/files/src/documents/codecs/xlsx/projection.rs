//! Projects worksheets into sheet, row and cell nodes, and renders the
//! HTML preview and plain-text views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash, hash8,
};
use umya_spreadsheet::{Cell, Worksheet};

use super::workbook::{
    CellRange, a1, cell_display, cell_type, display, load, node_id, row_text, rows, sheet_id,
};

const MAX_PROJECTED_CELLS: usize = 2_000;
const PREVIEW_ROWS: u32 = 200;
const PREVIEW_COLS: u32 = 50;

pub(super) fn project(bytes: &[u8]) -> Result<Projection, CodecError> {
    let workbook = load(bytes)?;
    let root = workbook
        .sheet_collection()
        .iter()
        .enumerate()
        .map(|(index, sheet)| sheet_node(index, sheet))
        .collect();
    Ok(Projection {
        format: DocumentFormat::Xlsx,
        root,
        content_sha256: content_hash(bytes),
    })
}

pub(super) fn project_range(
    bytes: &[u8],
    sheet_name: &str,
    a1_range: &str,
) -> Result<Vec<ProjectionNode>, CodecError> {
    let workbook = load(bytes)?;
    let sheet = workbook
        .sheet_by_name(sheet_name)
        .map_err(|_| CodecError::UnknownNode {
            node: node_id(sheet_id(sheet_name)),
        })?;
    let range = CellRange::parse(a1_range).ok_or(CodecError::Malformed {
        detail: "range is not an A1 reference",
    })?;
    let prefix = sheet_id(sheet.name());
    Ok(rows(sheet)
        .into_iter()
        .filter_map(|(row, cells)| {
            let inside: BTreeMap<u32, &Cell> = cells
                .into_iter()
                .filter(|(col, _)| range.contains(*col, row))
                .collect();
            (!inside.is_empty()).then(|| row_node(&prefix, row, &inside, true))
        })
        .collect())
}

fn sheet_node(index: usize, sheet: &Worksheet) -> ProjectionNode {
    let prefix = sheet_id(sheet.name());
    let mut emitted = 0_usize;
    let mut truncated = false;
    let children: Vec<ProjectionNode> = rows(sheet)
        .into_iter()
        .map(|(row, cells)| {
            emitted += cells.len();
            let with_cells = emitted <= MAX_PROJECTED_CELLS;
            truncated |= !with_cells;
            row_node(&prefix, row, &cells, with_cells)
        })
        .collect();
    let joined = children
        .iter()
        .map(|row| row.hash8.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut attrs = BTreeMap::from([
        (
            "dimension".to_owned(),
            sheet.calculate_worksheet_dimension(),
        ),
        ("index".to_owned(), index.to_string()),
    ]);
    if truncated {
        attrs.insert("truncated".to_owned(), "true".to_owned());
    }
    ProjectionNode {
        id: node_id(prefix),
        kind: NodeKind::Sheet,
        text: Some(sheet.name().to_owned()),
        hash8: hash8(&joined),
        children,
        attrs,
    }
}

fn row_node(
    prefix: &str,
    row: u32,
    cells: &BTreeMap<u32, &Cell>,
    with_cells: bool,
) -> ProjectionNode {
    let text = row_text(cells);
    let children = if with_cells {
        cells
            .iter()
            .map(|(col, cell)| cell_node(prefix, *col, row, cell))
            .collect()
    } else {
        Vec::new()
    };
    ProjectionNode {
        id: node_id(format!("{prefix}/row[{row}]")),
        kind: NodeKind::Row,
        hash8: hash8(&text),
        text: Some(text),
        children,
        attrs: BTreeMap::new(),
    }
}

fn cell_node(prefix: &str, col: u32, row: u32, cell: &Cell) -> ProjectionNode {
    let text = display(cell);
    let mut attrs = BTreeMap::from([("type".to_owned(), cell_type(cell).to_owned())]);
    if cell.is_formula() {
        attrs.insert("formula".to_owned(), cell.formula().to_owned());
    }
    ProjectionNode {
        id: node_id(format!("{prefix}/{}", a1(col, row))),
        kind: NodeKind::Cell,
        hash8: hash8(&text),
        text: Some(text),
        children: Vec::new(),
        attrs,
    }
}

pub(super) fn preview_html(bytes: &[u8]) -> Result<String, CodecError> {
    let workbook = load(bytes)?;
    let mut html = String::new();
    for sheet in workbook.sheet_collection() {
        let (max_col, max_row) = sheet.highest_column_and_row();
        html.push_str("<table><caption>");
        html.push_str(&escape(sheet.name()));
        html.push_str("</caption>");
        for row in 1..=max_row.min(PREVIEW_ROWS) {
            html.push_str("<tr>");
            for col in 1..=max_col.min(PREVIEW_COLS) {
                html.push_str("<td>");
                html.push_str(&escape(&cell_display(sheet, col, row)));
                html.push_str("</td>");
            }
            html.push_str("</tr>");
        }
        html.push_str("</table>\n");
    }
    Ok(html)
}

pub(super) fn plain_text(bytes: &[u8]) -> Result<String, CodecError> {
    let workbook = load(bytes)?;
    let mut text = String::new();
    for sheet in workbook.sheet_collection() {
        text.push_str("# ");
        text.push_str(sheet.name());
        text.push('\n');
        for cells in rows(sheet).values() {
            text.push_str(&row_text(cells));
            text.push('\n');
        }
    }
    Ok(text)
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}
