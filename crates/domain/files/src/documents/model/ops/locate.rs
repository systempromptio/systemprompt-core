//! A projection node mapped to the model block it stands for.
//!
//! A codec's projection and the `RichText` model of the same bytes list a
//! document's top-level blocks in the same order, under different ids. A node
//! inside a table maps to that table, counted among the tables of the node's
//! root; any other node maps to the block with the same text, counted among
//! the leaves of its root that carry that text. Text is compared without
//! Markdown syntax and with whitespace collapsed, so a Markdown source line
//! and the block parsed from it match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, NodeKind, Projection, ProjectionNode,
};

use crate::documents::model::{Block, BlockKind, RichText};

pub(super) fn block_index(
    ids: &Projection,
    model: &RichText,
    node: &DocumentNodeId,
) -> Result<usize, CodecError> {
    let unknown = || CodecError::UnknownNode { node: node.clone() };
    for root in &ids.root {
        let mut walk = Walk {
            target: node,
            tables: 0,
            leaves: Vec::new(),
            found: None,
        };
        walk.visit(root, None);
        let Some(found) = walk.found else {
            continue;
        };
        let blocks = model.blocks.iter().enumerate();
        let hit = match found {
            Found::Table(nth) => blocks.filter(|(_, block)| is_table(block)).nth(nth),
            Found::Leaf(key, nth) => blocks
                .filter(|(_, block)| !is_table(block) && plain(&block.text) == key)
                .nth(nth),
        };
        return hit.map(|(at, _)| at).ok_or_else(unknown);
    }
    Err(unknown())
}

pub(super) fn is_table(block: &Block) -> bool {
    match &block.kind {
        BlockKind::Table { .. } => true,
        BlockKind::Raw { source } => {
            let source = source.trim_start();
            source.starts_with("<w:tbl") || source.starts_with("<table")
        },
        _ => false,
    }
}

enum Found {
    Table(usize),
    Leaf(String, usize),
}

struct Walk<'a> {
    target: &'a DocumentNodeId,
    tables: usize,
    leaves: Vec<String>,
    found: Option<Found>,
}

impl Walk<'_> {
    fn visit(&mut self, node: &ProjectionNode, table: Option<usize>) {
        if self.found.is_some() {
            return;
        }
        let table = match (table, node.kind) {
            (None, NodeKind::Table) => {
                self.tables += 1;
                Some(self.tables - 1)
            },
            (held, _) => held,
        };
        let key = || plain(node.text.as_deref().unwrap_or_default());
        if &node.id == self.target {
            self.found = Some(match table {
                Some(nth) => Found::Table(nth),
                None => {
                    let key = key();
                    let nth = self.leaves.iter().filter(|leaf| **leaf == key).count();
                    Found::Leaf(key, nth)
                },
            });
            return;
        }
        if table.is_none() && node.children.is_empty() {
            self.leaves.push(key());
        }
        for child in &node.children {
            self.visit(child, table);
        }
    }
}

fn plain(text: &str) -> String {
    strip_marker(text.trim_start())
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`' | '~' | '\\'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_marker(line: &str) -> &str {
    let hashes = line.trim_start_matches('#');
    if hashes.len() < line.len() && hashes.starts_with(' ') {
        return hashes.trim_start();
    }
    for marker in ["> ", "- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return rest;
        }
    }
    let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
    if digits.len() < line.len()
        && let Some(rest) = digits
            .strip_prefix(". ")
            .or_else(|| digits.strip_prefix(") "))
    {
        return rest;
    }
    line
}
