//! Giving a freshly seeded rich document the projection's node ids, so an
//! agent op that names a node it read from `documents_read` finds the same
//! block, row or cell in the live session.
//!
//! The projection's block-level nodes (descending through `Body`, `Html`,
//! `Markdown` and `Text` containers) are aligned with the root block list
//! in order, a table's rows and cells with the table node's `Row` and
//! `Cell` children, and a cell's blocks with the cell node's children. The
//! ids are written only when the whole tree aligns, count for count and
//! kind for kind; otherwise the seed keeps its own ids and nothing is
//! written, so a format whose projection is not block-shaped (Markdown
//! lines) is left as it was.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroDoc, LoroMap, LoroMovableList};
use systemprompt_provider_contracts::documents::{NodeKind, ProjectionNode};

use super::rich::{WriteError, block_map, blocks, list_of};
use super::table::is_table_map;

/// Writes the projection's ids into `doc` when they align; answers whether
/// they did.
pub fn adopt(doc: &LoroDoc, nodes: &[ProjectionNode]) -> Result<bool, WriteError> {
    let flat = block_nodes(nodes);
    let list = blocks(doc);
    if !aligns(&list, &flat) {
        return Ok(false);
    }
    write_list(&list, &flat)?;
    Ok(true)
}

fn block_nodes(nodes: &[ProjectionNode]) -> Vec<&ProjectionNode> {
    let mut out = Vec::new();
    for node in nodes {
        match node.kind {
            NodeKind::Body | NodeKind::Html | NodeKind::Markdown | NodeKind::Text => {
                out.extend(block_nodes(&node.children));
            },
            _ => out.push(node),
        }
    }
    out
}

fn entries(list: &LoroMovableList) -> Option<Vec<LoroMap>> {
    (0..list.len()).map(|at| block_map(list, at)).collect()
}

fn aligns(list: &LoroMovableList, nodes: &[&ProjectionNode]) -> bool {
    let Some(maps) = entries(list) else {
        return false;
    };
    maps.len() == nodes.len()
        && maps.iter().zip(nodes).all(|(map, node)| match node.kind {
            NodeKind::Table => is_table_map(map) && table_aligns(map, node),
            NodeKind::Paragraph
            | NodeKind::Heading { .. }
            | NodeKind::ListItem
            | NodeKind::Image
            | NodeKind::Pending => !is_table_map(map),
            _ => false,
        })
}

fn table_aligns(table: &LoroMap, node: &ProjectionNode) -> bool {
    let Some(rows) = list_of(table, "rows").as_ref().and_then(entries) else {
        return false;
    };
    rows.len() == node.children.len()
        && rows.iter().zip(&node.children).all(|(row, row_node)| {
            let Some(cells) = list_of(row, "cells").as_ref().and_then(entries) else {
                return false;
            };
            row_node.kind == NodeKind::Row
                && cells.len() == row_node.children.len()
                && cells
                    .iter()
                    .zip(&row_node.children)
                    .all(|(cell, cell_node)| {
                        let inner: Vec<&ProjectionNode> = cell_node.children.iter().collect();
                        cell_node.kind == NodeKind::Cell
                            && list_of(cell, "blocks").is_some_and(|list| aligns(&list, &inner))
                    })
        })
}

fn write_list(list: &LoroMovableList, nodes: &[&ProjectionNode]) -> Result<(), WriteError> {
    for (at, node) in nodes.iter().enumerate() {
        let Some(map) = block_map(list, at) else {
            continue;
        };
        set_id(&map, node)?;
        if node.kind != NodeKind::Table {
            continue;
        }
        let Some(rows) = list_of(&map, "rows") else {
            continue;
        };
        for (r, row_node) in node.children.iter().enumerate() {
            let Some(row) = block_map(&rows, r) else {
                continue;
            };
            set_id(&row, row_node)?;
            let Some(cells) = list_of(&row, "cells") else {
                continue;
            };
            for (c, cell_node) in row_node.children.iter().enumerate() {
                let Some(cell) = block_map(&cells, c) else {
                    continue;
                };
                set_id(&cell, cell_node)?;
                if let Some(inner) = list_of(&cell, "blocks") {
                    let inner_nodes: Vec<&ProjectionNode> = cell_node.children.iter().collect();
                    write_list(&inner, &inner_nodes)?;
                }
            }
        }
    }
    Ok(())
}

fn set_id(map: &LoroMap, node: &ProjectionNode) -> Result<(), WriteError> {
    let id = node.id.as_str();
    if super::rich::string_of(map, "id").as_deref() != Some(id) {
        map.insert("id", id)?;
    }
    Ok(())
}
