//! The `.odt` document model: body paragraphs, lists and tables read from
//! `office:text`, their projection, preview and plain text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::ops::Range;

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::{CodecError, NodeKind, ProjectionNode};

use super::apply::Container;
use super::package::Content;
use super::projection::{Flow, Para, container_node, is_para, para_node, preview_paras};
use crate::documents::codecs::xml_tree::Element;

const SECTIONS: [&str; 3] = ["text:section", "text:index-body", "text:soft-page-break"];
const ROW_GROUPS: [&str; 3] = [
    "table:table-header-rows",
    "table:table-rows",
    "table:table-row-group",
];

#[derive(Debug)]
pub(super) enum Block {
    Paras(Range<usize>),
    Table(Table),
}

#[derive(Debug)]
pub(super) struct Table {
    pub id: String,
    pub range: Range<usize>,
    pub rows: Vec<Row>,
}

#[derive(Debug)]
pub(super) struct Row {
    pub id: String,
    pub cells: Vec<Cell>,
}

#[derive(Debug)]
pub(super) struct Cell {
    pub id: String,
    pub paras: Range<usize>,
}

#[derive(Debug)]
pub(super) struct TextDoc {
    pub paras: Vec<Para>,
    pub blocks: Vec<Block>,
}

struct Walker<'x> {
    flow: Flow<'x>,
    xml: &'x str,
    blocks: Vec<Block>,
    tables: usize,
}

impl Walker<'_> {
    fn blocks(&mut self, parent: &Element) -> Result<(), CodecError> {
        for child in &parent.children {
            let first = self.flow.paras.len();
            if is_para(child) {
                self.flow.para(child, None)?;
            } else if child.name == "text:list" {
                self.flow.list(child)?;
            } else if child.name == "table:table" {
                let table = self.table(child)?;
                self.blocks.push(Block::Table(table));
                continue;
            } else if SECTIONS.contains(&child.name.as_str()) {
                self.blocks(child)?;
                continue;
            }
            let last = self.flow.paras.len();
            if last > first {
                self.blocks.push(Block::Paras(first..last));
            }
        }
        Ok(())
    }

    fn table(&mut self, element: &Element) -> Result<Table, CodecError> {
        let id = format!("body/table[{}]", self.tables);
        self.tables += 1;
        let mut row_elements = Vec::new();
        collect_rows(element, &mut row_elements);
        let mut rows = Vec::new();
        for (r, row) in row_elements.into_iter().enumerate() {
            let row_id = format!("{id}/tr[{r}]");
            let mut cells = Vec::new();
            for (c, cell) in row
                .children
                .iter()
                .filter(|cell| cell.name == "table:table-cell")
                .enumerate()
            {
                let cell_id = format!("{row_id}/tc[{c}]");
                let mut flow = Flow::new(self.xml, cell_id.clone());
                flow.text_container(cell)?;
                let first = self.flow.paras.len();
                self.flow.paras.extend(flow.paras);
                cells.push(Cell {
                    id: cell_id,
                    paras: first..self.flow.paras.len(),
                });
            }
            rows.push(Row { id: row_id, cells });
        }
        Ok(Table {
            id,
            range: element.start..element.end,
            rows,
        })
    }
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

pub(super) fn read(content: &Content) -> Result<TextDoc, CodecError> {
    let body = content.body("office:text")?;
    let mut walker = Walker {
        flow: Flow::new(&content.xml, "body"),
        xml: &content.xml,
        blocks: Vec::new(),
        tables: 0,
    };
    walker.blocks(body)?;
    Ok(TextDoc {
        paras: walker.flow.paras,
        blocks: walker.blocks,
    })
}

impl TextDoc {
    fn cell_text(&self, cell: &Cell) -> String {
        self.paras[cell.paras.clone()]
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn row_text(&self, row: &Row) -> String {
        row.cells
            .iter()
            .map(|cell| self.cell_text(cell))
            .collect::<Vec<_>>()
            .join("\t")
    }

    fn table_text(&self, table: &Table) -> String {
        table
            .rows
            .iter()
            .map(|row| self.row_text(row))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(super) fn containers(&self) -> Vec<Container> {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                Block::Table(table) => Some(Container {
                    id: table.id.clone(),
                    text: self.table_text(table),
                    range: table.range.clone(),
                    accepts_insert_after: true,
                }),
                Block::Paras(_) => None,
            })
            .collect()
    }

    pub(super) fn nodes(&self) -> Vec<ProjectionNode> {
        let mut out = Vec::new();
        for block in &self.blocks {
            match block {
                Block::Paras(range) => out.extend(self.paras[range.clone()].iter().map(para_node)),
                Block::Table(table) => out.push(self.table_node(table)),
            }
        }
        out
    }

    fn table_node(&self, table: &Table) -> ProjectionNode {
        let rows = table
            .rows
            .iter()
            .map(|row| {
                let cells = row
                    .cells
                    .iter()
                    .map(|cell| {
                        let text = self.cell_text(cell);
                        let paras = self.paras[cell.paras.clone()]
                            .iter()
                            .map(para_node)
                            .collect();
                        container_node(
                            &cell.id,
                            NodeKind::Cell,
                            Some(text.clone()),
                            &text,
                            paras,
                            BTreeMap::new(),
                        )
                    })
                    .collect();
                container_node(
                    &row.id,
                    NodeKind::Row,
                    None,
                    &self.row_text(row),
                    cells,
                    BTreeMap::new(),
                )
            })
            .collect();
        container_node(
            &table.id,
            NodeKind::Table,
            None,
            &self.table_text(table),
            rows,
            BTreeMap::new(),
        )
    }

    pub(super) fn plain_text(&self) -> String {
        self.blocks
            .iter()
            .map(|block| match block {
                Block::Paras(range) => self.paras[range.clone()]
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                Block::Table(table) => self.table_text(table),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(super) fn preview(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            match block {
                Block::Paras(range) => preview_paras(&self.paras[range.clone()], &mut out),
                Block::Table(table) => {
                    out.push_str("<table>");
                    for row in &table.rows {
                        out.push_str("<tr>");
                        for cell in &row.cells {
                            out.push_str("<td>");
                            out.push_str(&escape(self.cell_text(cell).as_str()));
                            out.push_str("</td>");
                        }
                        out.push_str("</tr>");
                    }
                    out.push_str("</table>\n");
                },
            }
        }
        out
    }
}
