//! Projection of scanned blocks into hash-anchored nodes, and the plain-text
//! rendering `str_replace` matches against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    DocumentFormat, NodeKind, Projection, ProjectionNode, content_hash, hash8,
};

use crate::documents::codecs::pending::intent_of_line;

use super::scan::{Block, Cell, Doc, Para, Table};

pub(super) fn project(bytes: &[u8], doc: &Doc<'_>) -> Projection {
    Projection {
        format: DocumentFormat::Docx,
        root: doc.blocks.iter().map(block_node).collect(),
        content_sha256: content_hash(bytes),
    }
}

pub(super) fn heading_level(style: &str) -> Option<u8> {
    if style.eq_ignore_ascii_case("title") {
        return Some(1);
    }
    let digits = style
        .get(..7)
        .filter(|head| head.eq_ignore_ascii_case("heading"))
        .and_then(|_| style.get(7..))?;
    match digits.parse::<u8>() {
        Ok(level @ 1..=9) => Some(level),
        _ => None,
    }
}

pub(super) fn para_kind(para: &Para) -> NodeKind {
    let props = para.ppr.as_ref();
    if let Some(level) = props
        .and_then(|p| p.style.as_deref())
        .and_then(heading_level)
    {
        return NodeKind::Heading { level };
    }
    if props.is_some_and(|p| p.num_range.is_some()) {
        NodeKind::ListItem
    } else {
        NodeKind::Paragraph
    }
}

fn block_node(block: &Block) -> ProjectionNode {
    match block {
        Block::Para(para) => para_node(para),
        Block::Table(table) => table_node(table),
    }
}

fn para_node(para: &Para) -> ProjectionNode {
    let text = para.text();
    let mut attrs = BTreeMap::new();
    if let Some(pid) = &para.w14_id {
        attrs.insert("para_id".to_owned(), pid.clone());
    }
    if let Some(style) = para.ppr.as_ref().and_then(|p| p.style.clone()) {
        attrs.insert("style".to_owned(), style);
    }
    let children = para
        .images
        .iter()
        .enumerate()
        .map(|(index, rel_id)| ProjectionNode {
            id: DocumentNodeId::new(format!("{}/img[{index}]", para.id)),
            kind: NodeKind::Image,
            text: None,
            hash8: hash8(rel_id),
            children: Vec::new(),
            attrs: BTreeMap::from([("rel_id".to_owned(), rel_id.clone())]),
        })
        .collect();
    let hash = hash8(&text);
    let (kind, text) = match intent_of_line(&text) {
        Some(intent) => (NodeKind::Pending, intent.to_owned()),
        None => (para_kind(para), text),
    };
    ProjectionNode {
        id: DocumentNodeId::new(para.id.clone()),
        kind,
        hash8: hash,
        text: Some(text),
        children,
        attrs,
    }
}

fn container(
    id: &str,
    kind: NodeKind,
    text: &str,
    children: Vec<ProjectionNode>,
) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id.to_owned()),
        kind,
        text: None,
        hash8: hash8(text),
        children,
        attrs: BTreeMap::new(),
    }
}

fn table_node(table: &Table) -> ProjectionNode {
    let rows = table
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let cells = row.iter().map(cell_node).collect();
            container(
                &format!("{}/tr[{index}]", table.id),
                NodeKind::Row,
                &row_text(row),
                cells,
            )
        })
        .collect();
    container(&table.id, NodeKind::Table, &table_text(table), rows)
}

fn cell_node(cell: &Cell) -> ProjectionNode {
    let text = blocks_text(&cell.blocks);
    ProjectionNode {
        text: Some(text.clone()),
        ..container(
            &cell.id,
            NodeKind::Cell,
            &text,
            cell.blocks.iter().map(block_node).collect(),
        )
    }
}

fn row_text(row: &[Cell]) -> String {
    row.iter()
        .map(|cell| blocks_text(&cell.blocks))
        .collect::<Vec<_>>()
        .join("\t")
}

pub(super) fn table_text(table: &Table) -> String {
    table
        .rows
        .iter()
        .map(|row| row_text(row))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn blocks_text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            Block::Para(para) => para.text(),
            Block::Table(table) => table_text(table),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
