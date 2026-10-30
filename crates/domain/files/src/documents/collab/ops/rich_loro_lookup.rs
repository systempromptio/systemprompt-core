//! Live-tree lookups behind the Loro text ops: a block by id or unique
//! quote (searched through table cells, nested tables included), its live text
//! under an `expected_hash` guard, and the rows and columns of a live table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroDoc, LoroMovableList};
use systemprompt_provider_contracts::documents::{MarkSpec, NodeTarget, hash8};

use super::rich_loro::RichOpError;
use crate::documents::engine::loro::edit::{self, Located, find_block};
use crate::documents::engine::loro::rich::{
    block_map, blocks, list_of, read_blocks, string_of, text_of,
};
use crate::documents::model::{Block, MarkKind};

pub(super) fn located(doc: &LoroDoc, id: &str) -> Result<Located, RichOpError> {
    find_block(doc, id).ok_or_else(|| RichOpError::UnknownBlock {
        node: id.to_owned(),
    })
}

pub(super) fn live_text(doc: &LoroDoc, id: &str) -> Result<String, RichOpError> {
    Ok(text_of(&located(doc, id)?.map)
        .map(|text| text.to_string())
        .unwrap_or_default())
}

pub(super) fn guarded(
    doc: &LoroDoc,
    node: &NodeTarget,
    expected: Option<&str>,
) -> Result<String, RichOpError> {
    let id = match node {
        NodeTarget::Id(id) => id.as_str().to_owned(),
        NodeTarget::Find(quote) => unique_quote(doc, &quote.find)?.0,
    };
    let text = live_text(doc, &id)?;
    if let Some(expected) = expected {
        let actual = hash8(&text);
        if actual != expected {
            return Err(RichOpError::StaleAnchor {
                node: id,
                expected: expected.to_owned(),
                actual,
                current_text: text,
            });
        }
    }
    Ok(id)
}

pub(super) fn unique_quote(doc: &LoroDoc, find: &str) -> Result<(String, String), RichOpError> {
    let mut hits = Vec::new();
    quote_hits(&blocks(doc), find, &mut hits);
    let total: usize = hits.iter().map(|(_, _, count)| count).sum();
    match hits.into_iter().next() {
        Some((id, text, 1)) if total == 1 => Ok((id, text)),
        _ => Err(RichOpError::Ambiguous {
            find: find.to_owned(),
            matches: total,
        }),
    }
}

fn quote_hits(list: &LoroMovableList, find: &str, hits: &mut Vec<(String, String, usize)>) {
    for at in 0..list.len() {
        let Some(map) = block_map(list, at) else {
            continue;
        };
        if let Some(text) = text_of(&map).map(|text| text.to_string()) {
            let count = text.matches(find).count();
            if count > 0 {
                hits.push((string_of(&map, "id").unwrap_or_default(), text, count));
            }
        }
        for cell in edit::cells_of(&map) {
            if let Some(inner) = list_of(&cell, "blocks") {
                quote_hits(&inner, find, hits);
            }
        }
    }
}

pub(super) fn nth_range(text: &str, find: &str, nth: usize) -> Option<std::ops::Range<usize>> {
    let (byte, _) = text.match_indices(find).nth(nth.saturating_sub(1))?;
    let start = text[..byte].chars().count();
    Some(start..start + find.chars().count())
}

pub(super) fn mark_kind(spec: &MarkSpec) -> Option<MarkKind> {
    MarkKind::from_key(&spec.mark, spec.value.as_deref().unwrap_or_default())
}

pub(super) fn read_block(doc: &LoroDoc, id: &str) -> Result<Block, RichOpError> {
    let Located { list, at, .. } = located(doc, id)?;
    read_blocks(&list)
        .ok()
        .and_then(|mut read| (at < read.len()).then(|| read.swap_remove(at)))
        .ok_or_else(|| RichOpError::UnknownBlock {
            node: id.to_owned(),
        })
}

pub(super) fn rows_len(doc: &LoroDoc, table: &str) -> Result<usize, RichOpError> {
    Ok(list_of(&located(doc, table)?.map, "rows").map_or(0, |rows| rows.len()))
}

pub(super) fn columns(doc: &LoroDoc, table: &str) -> Result<usize, RichOpError> {
    let map = located(doc, table)?.map;
    Ok(edit::rows_of(&map)
        .next()
        .and_then(|row| list_of(&row, "cells"))
        .map_or(0, |cells| cells.len()))
}

pub(super) fn row_id(doc: &LoroDoc, table: &str, row: usize) -> Result<String, RichOpError> {
    let map = located(doc, table)?.map;
    edit::rows_of(&map)
        .nth(row)
        .and_then(|row| string_of(&row, "id"))
        .ok_or_else(|| RichOpError::UnknownBlock {
            node: format!("{table} row {row}"),
        })
}
