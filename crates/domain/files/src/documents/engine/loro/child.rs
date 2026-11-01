//! Lazily created map-key children, identified by `(parent, key, type)` so
//! two replicas creating the same child at once write into one container.
//!
//! A child is taken as it is when the key already holds a container of the
//! wanted type, so a document written before mergeable children still reads
//! and edits; otherwise it is created with Loro's `ensure_mergeable_*`, as
//! the browser replica does. [`has_legacy_children`] finds the older regular
//! children a table still holds, and [`fresh_map`] and [`fresh_list`] replace
//! one so the write-back that follows refills it as a mergeable child.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{Container, ContainerTrait, LoroDoc, LoroMap, LoroMovableList, LoroResult};

use super::rich::{block_map, blocks, list_of, map_of};
use super::table::is_table_map;

pub(crate) fn child_map(parent: &LoroMap, key: &str) -> LoroResult<LoroMap> {
    map_of(parent, key).map_or_else(|| parent.ensure_mergeable_map(key), Ok)
}

pub(crate) fn child_list(parent: &LoroMap, key: &str) -> LoroResult<LoroMovableList> {
    list_of(parent, key).map_or_else(|| parent.ensure_mergeable_movable_list(key), Ok)
}

pub(crate) fn fresh_map(parent: &LoroMap, key: &str) -> LoroResult<LoroMap> {
    drop_legacy(parent, key)?;
    child_map(parent, key)
}

pub(crate) fn fresh_list(parent: &LoroMap, key: &str) -> LoroResult<LoroMovableList> {
    drop_legacy(parent, key)?;
    child_list(parent, key)
}

#[must_use]
pub(crate) fn is_legacy(parent: &LoroMap, key: &str) -> bool {
    match parent.get(key) {
        Some(loro::ValueOrContainer::Container(child)) => !child_id_is_mergeable(&child),
        _ => false,
    }
}

fn child_id_is_mergeable(child: &Container) -> bool {
    child.id().is_mergeable()
}

fn drop_legacy(parent: &LoroMap, key: &str) -> LoroResult<()> {
    if is_legacy(parent, key) {
        parent.delete(key)?;
    }
    Ok(())
}

#[must_use]
pub(crate) fn has_legacy_children(doc: &LoroDoc) -> bool {
    legacy_in(&blocks(doc))
}

fn legacy_in(list: &LoroMovableList) -> bool {
    (0..list.len())
        .any(|at| block_map(list, at).is_some_and(|map| is_table_map(&map) && legacy_table(&map)))
}

fn legacy_table(table: &LoroMap) -> bool {
    if is_legacy(table, "props") || is_legacy(table, "rows") {
        return true;
    }
    let Some(rows) = list_of(table, "rows") else {
        return false;
    };
    (0..rows.len())
        .filter_map(|r| block_map(&rows, r))
        .any(|row| {
            if is_legacy(&row, "cells") {
                return true;
            }
            let Some(cells) = list_of(&row, "cells") else {
                return false;
            };
            (0..cells.len())
                .filter_map(|c| block_map(&cells, c))
                .any(|cell| {
                    is_legacy(&cell, "props")
                        || is_legacy(&cell, "blocks")
                        || list_of(&cell, "blocks").is_some_and(|inner| legacy_in(&inner))
                })
        })
}
