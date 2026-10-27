//! Carrying node ids from one revision's projection to the next. Ids the
//! commit deleted are retired before alignment, so a block inserted where a
//! deleted one stood is minted rather than inheriting its id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::mem::discriminant;

use systemprompt_identifiers::DocumentNodeId;

use systemprompt_models::documents::{Projection, ProjectionNode};

use super::flat::flatten;
use super::minter::NodeIdMinter;
use crate::documents::models::NodeIdEntry;

const MAX_TABLE_CELLS: usize = 4_000_000;
const GREEDY_WINDOW: usize = 64;

#[must_use]
pub fn mint_all(next: &Projection, minter: &mut NodeIdMinter) -> Vec<NodeIdEntry> {
    flatten(next)
        .into_iter()
        .map(|flat| NodeIdEntry {
            node_id: minter.mint(),
            path: flat.node.id.clone(),
            hash8: flat.node.hash8.clone(),
        })
        .collect()
}

#[must_use]
pub fn carry_forward<S: std::hash::BuildHasher>(
    previous: &Projection,
    next: &Projection,
    retired: &HashSet<DocumentNodeId, S>,
    minter: &mut NodeIdMinter,
) -> Vec<NodeIdEntry> {
    let old: Vec<&ProjectionNode> = flatten(previous)
        .into_iter()
        .map(|f| f.node)
        .filter(|node| !retired.contains(&node.id))
        .collect();
    let new: Vec<&ProjectionNode> = flatten(next).into_iter().map(|f| f.node).collect();
    let mut matched: Vec<Option<usize>> = vec![None; new.len()];
    let anchors = align(&old, &new);
    let mut old_from = 0;
    let mut new_from = 0;
    for &(o, n) in anchors
        .iter()
        .chain(std::iter::once(&(old.len(), new.len())))
    {
        pair_gap(&old, &new, (old_from, o), (new_from, n), &mut matched);
        if n < new.len() {
            matched[n] = Some(o);
        }
        old_from = o + 1;
        new_from = n + 1;
    }
    new.iter()
        .zip(matched)
        .map(|(node, old_index)| NodeIdEntry {
            node_id: old_index.map_or_else(|| minter.mint(), |o| old[o].id.clone()),
            path: node.id.clone(),
            hash8: node.hash8.clone(),
        })
        .collect()
}

fn same_key(a: &ProjectionNode, b: &ProjectionNode) -> bool {
    a.kind == b.kind && a.hash8 == b.hash8
}

fn same_kind(a: &ProjectionNode, b: &ProjectionNode) -> bool {
    discriminant(&a.kind) == discriminant(&b.kind)
}

fn pair_gap(
    old: &[&ProjectionNode],
    new: &[&ProjectionNode],
    (old_from, old_to): (usize, usize),
    (new_from, new_to): (usize, usize),
    matched: &mut [Option<usize>],
) {
    let old_len = old_to.saturating_sub(old_from);
    let new_len = new_to.saturating_sub(new_from);
    let mut front = 0;
    while front < old_len.min(new_len) && same_kind(old[old_from + front], new[new_from + front]) {
        matched[new_from + front] = Some(old_from + front);
        front += 1;
    }
    let mut back = 0;
    while front + back < old_len.min(new_len)
        && same_kind(old[old_to - 1 - back], new[new_to - 1 - back])
    {
        matched[new_to - 1 - back] = Some(old_to - 1 - back);
        back += 1;
    }
}

fn align(old: &[&ProjectionNode], new: &[&ProjectionNode]) -> Vec<(usize, usize)> {
    let mut prefix = 0;
    while prefix < old.len() && prefix < new.len() && same_key(old[prefix], new[prefix]) {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old.len() - prefix
        && suffix < new.len() - prefix
        && same_key(old[old.len() - 1 - suffix], new[new.len() - 1 - suffix])
    {
        suffix += 1;
    }
    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];
    let middle = if old_mid.len().saturating_mul(new_mid.len()) <= MAX_TABLE_CELLS {
        lcs(old_mid, new_mid)
    } else {
        greedy(old_mid, new_mid)
    };
    (0..prefix)
        .map(|i| (i, i))
        .chain(middle.into_iter().map(|(o, n)| (o + prefix, n + prefix)))
        .chain((0..suffix).map(|i| (old.len() - suffix + i, new.len() - suffix + i)))
        .collect()
}

fn lcs(old: &[&ProjectionNode], new: &[&ProjectionNode]) -> Vec<(usize, usize)> {
    let width = new.len() + 1;
    let mut table = vec![0u32; (old.len() + 1) * width];
    for o in (0..old.len()).rev() {
        for n in (0..new.len()).rev() {
            table[o * width + n] = if same_key(old[o], new[n]) {
                table[(o + 1) * width + n + 1] + 1
            } else {
                table[(o + 1) * width + n].max(table[o * width + n + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut o, mut n) = (0, 0);
    while o < old.len() && n < new.len() {
        if same_key(old[o], new[n]) {
            pairs.push((o, n));
            o += 1;
            n += 1;
        } else if table[(o + 1) * width + n] >= table[o * width + n + 1] {
            o += 1;
        } else {
            n += 1;
        }
    }
    pairs
}

fn greedy(old: &[&ProjectionNode], new: &[&ProjectionNode]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut cursor = 0;
    for (n, node) in new.iter().enumerate() {
        let end = (cursor + GREEDY_WINDOW).min(old.len());
        if let Some(offset) = old[cursor..end].iter().position(|o| same_key(o, node)) {
            pairs.push((cursor + offset, n));
            cursor += offset + 1;
        }
    }
    pairs
}
