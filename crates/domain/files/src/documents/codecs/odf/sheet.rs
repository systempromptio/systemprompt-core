//! The `.ods` sheet model: `table:table` sheets whose rows and cells carry
//! their `number-rows-repeated`/`number-columns-repeated` runs unexpanded,
//! addressed virtually by A1 position; the projection, preview and plain
//! text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::ops::Range;

use systemprompt_provider_contracts::documents::{CodecError, NodeKind, ProjectionNode};

use super::a1::{a1, sheet_id};
use super::package::{Content, inline_text};
use super::projection::{ContainerSpec, container_node};
use crate::documents::codecs::xml_tree::Element;

const MAX_PROJECTED_CELLS: usize = 2_000;
pub(super) const PREVIEW_ROWS: u32 = 200;
pub(super) const PREVIEW_COLS: u32 = 50;
pub(super) const MAX_TEXT_CELLS: usize = 100_000;
const ROW_GROUPS: [&str; 3] = [
    "table:table-header-rows",
    "table:table-rows",
    "table:table-row-group",
];

#[derive(Debug, Clone, Default)]
pub(super) struct CellData {
    pub display: String,
    pub value_type: Option<String>,
    pub value: Option<String>,
    pub formula: Option<String>,
}

impl CellData {
    const fn is_empty(&self) -> bool {
        self.display.is_empty() && self.formula.is_none() && self.value.is_none()
    }
}

pub(super) type FilledRow<'a> = (u32, Vec<(u32, &'a CellData)>);

#[derive(Debug)]
pub(super) struct CellSeg {
    pub start_col: u32,
    pub repeat: u32,
    pub element: Element,
    pub data: CellData,
}

#[derive(Debug)]
pub(super) struct RowSeg {
    pub start_row: u32,
    pub repeat: u32,
    pub element: Element,
    pub cells: Vec<CellSeg>,
}

#[derive(Debug)]
pub(super) struct Sheet {
    pub name: String,
    pub rows: Vec<RowSeg>,
    pub rows_end: usize,
}

fn repeat(element: &Element, attr: &str) -> u32 {
    element
        .attr(attr)
        .and_then(|v| v.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(1)
}

fn cell_data(xml: &str, cell: &Element) -> Result<CellData, CodecError> {
    let mut lines = Vec::new();
    for para in cell.children_named("text:p") {
        lines.push(inline_text(xml, para)?);
    }
    let value_type = cell.attr("office:value-type").map(str::to_owned);
    let value_attr = match value_type.as_deref() {
        Some("date") => "office:date-value",
        Some("time") => "office:time-value",
        Some("boolean") => "office:boolean-value",
        Some("string") => "office:string-value",
        _ => "office:value",
    };
    Ok(CellData {
        display: lines.join("\n"),
        value: cell.attr(value_attr).map(str::to_owned),
        formula: cell.attr("table:formula").map(str::to_owned),
        value_type,
    })
}

fn collect_rows<'e>(element: &'e Element, out: &mut Vec<&'e Element>) {
    for child in &element.children {
        if child.name == "table:table-row" {
            out.push(child);
        } else if ROW_GROUPS.contains(&child.name.as_str()) {
            collect_rows(child, out);
        }
    }
}

pub(super) fn read(content: &Content) -> Result<Vec<Sheet>, CodecError> {
    let body = content.body("office:spreadsheet")?;
    let mut sheets = Vec::new();
    for table in body.children_named("table:table") {
        let mut row_elements = Vec::new();
        collect_rows(table, &mut row_elements);
        let rows_end = row_elements.last().map_or(table.inner_end, |row| row.end);
        let mut rows = Vec::new();
        let mut next_row = 1u32;
        for row in row_elements {
            let mut cells = Vec::new();
            let mut next_col = 1u32;
            for cell in row.children.iter().filter(|c| {
                matches!(
                    c.name.as_str(),
                    "table:table-cell" | "table:covered-table-cell"
                )
            }) {
                let count = repeat(cell, "table:number-columns-repeated");
                cells.push(CellSeg {
                    start_col: next_col,
                    repeat: count,
                    element: cell.clone(),
                    data: cell_data(&content.xml, cell)?,
                });
                next_col = next_col.saturating_add(count);
            }
            let count = repeat(row, "table:number-rows-repeated");
            rows.push(RowSeg {
                start_row: next_row,
                repeat: count,
                element: Element {
                    children: Vec::new(),
                    ..row.clone()
                },
                cells,
            });
            next_row = next_row.saturating_add(count);
        }
        sheets.push(Sheet {
            name: table.attr("table:name").unwrap_or_default().to_owned(),
            rows,
            rows_end,
        });
    }
    Ok(sheets)
}

impl Sheet {
    pub(super) fn cell(&self, col: u32, row: u32) -> Option<&CellData> {
        let seg = self
            .rows
            .iter()
            .find(|s| (s.start_row..s.start_row + s.repeat).contains(&row))?;
        seg.cells
            .iter()
            .find(|c| (c.start_col..c.start_col.saturating_add(c.repeat)).contains(&col))
            .map(|c| &c.data)
    }

    pub(super) fn display(&self, col: u32, row: u32) -> String {
        self.cell(col, row)
            .map(|c| c.display.clone())
            .unwrap_or_default()
    }

    pub(super) fn range_text(&self, cols: &Range<u32>, rows: &Range<u32>) -> String {
        rows.clone()
            .map(|row| {
                cols.clone()
                    .map(|col| self.display(col, row))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(super) fn filled(&self, cap: usize) -> (Vec<FilledRow<'_>>, bool) {
        let mut out = Vec::new();
        let mut emitted = 0usize;
        for seg in &self.rows {
            let cells: Vec<&CellSeg> = seg.cells.iter().filter(|c| !c.data.is_empty()).collect();
            if cells.is_empty() {
                continue;
            }
            for row in seg.start_row..seg.start_row.saturating_add(seg.repeat) {
                let mut row_cells = Vec::new();
                for cell in &cells {
                    for col in cell.start_col..cell.start_col.saturating_add(cell.repeat) {
                        if emitted >= cap {
                            out.push((row, row_cells));
                            return (out, true);
                        }
                        emitted += 1;
                        row_cells.push((col, &cell.data));
                    }
                }
                out.push((row, row_cells));
            }
        }
        (out, false)
    }

    pub(super) fn node(&self, index: usize) -> ProjectionNode {
        let prefix = sheet_id(&self.name);
        let (rows, truncated) = self.filled(MAX_PROJECTED_CELLS);
        let children: Vec<ProjectionNode> = rows
            .iter()
            .map(|(row, cells)| {
                let text = cells
                    .iter()
                    .map(|(_, c)| c.display.as_str())
                    .collect::<Vec<_>>()
                    .join("\t");
                let nodes = cells
                    .iter()
                    .map(|(col, data)| {
                        let mut attrs = BTreeMap::new();
                        for (key, value) in [
                            ("type", &data.value_type),
                            ("value", &data.value),
                            ("formula", &data.formula),
                        ] {
                            if let Some(value) = value {
                                attrs.insert(key.to_owned(), value.clone());
                            }
                        }
                        let id = format!("{prefix}/{}", a1(*col, *row));
                        container_node(
                            ContainerSpec {
                                id: &id,
                                kind: NodeKind::Cell,
                                text: Some(data.display.clone()),
                                hashed: &data.display,
                            },
                            Vec::new(),
                            attrs,
                        )
                    })
                    .collect();
                container_node(
                    ContainerSpec {
                        id: &format!("{prefix}/row[{row}]"),
                        kind: NodeKind::Row,
                        text: Some(text.clone()),
                        hashed: &text,
                    },
                    nodes,
                    BTreeMap::new(),
                )
            })
            .collect();
        let joined = children
            .iter()
            .map(|row| row.hash8.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let mut attrs = BTreeMap::from([("index".to_owned(), index.to_string())]);
        if truncated {
            attrs.insert("truncated".to_owned(), "true".to_owned());
        }
        container_node(
            ContainerSpec {
                id: &prefix,
                kind: NodeKind::Sheet,
                text: Some(self.name.clone()),
                hashed: &joined,
            },
            children,
            attrs,
        )
    }
}
