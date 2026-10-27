//! Carrying node ids from one revision's projection to the next. Ids the
//! commit deleted are retired before alignment, so a block inserted where a
//! deleted one stood is minted rather than inheriting its id.
//!
//! Unchanged nodes are aligned by a longest common subsequence of
//! `(kind, hash)`. Between aligned nodes, one node replaced by one node of
//! the same kind is an edit and keeps its id; in a wider gap an edited node
//! keeps its id only when it is the same kind and its text is similar to the
//! old node's (token Jaccard of at least 0.6; textless containers pair by
//! kind), or when the commit rewrote it in place; anything else is minted. A
//! node that moved keeps its id when an unpaired old node has the same kind and
//! hash.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::mem::discriminant;

use systemprompt_identifiers::DocumentNodeId;

use systemprompt_models::documents::{Projection, ProjectionNode};

use super::flat::flatten;
use super::minter::NodeIdMinter;
use crate::documents::models::NodeIdEntry;

const MAX_TABLE_CELLS: usize = 4_000_000;
const GREEDY_WINDOW: usize = 64;
const SIMILARITY: f64 = 0.6;

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

/// What a commit's ops say about the ids it touched: `retired` ids were
/// deleted, `rewritten` ids had their content replaced in place and keep
/// their id however different the new text is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarryHints {
    pub retired: HashSet<DocumentNodeId>,
    pub rewritten: HashSet<DocumentNodeId>,
}

#[must_use]
pub fn carry_forward(
    previous: &Projection,
    next: &Projection,
    hints: &CarryHints,
    minter: &mut NodeIdMinter,
) -> Vec<NodeIdEntry> {
    let old: Vec<&ProjectionNode> = flatten(previous)
        .into_iter()
        .map(|f| f.node)
        .filter(|node| !hints.retired.contains(&node.id))
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
        pair_gap(
            Gap {
                old: &old,
                new: &new,
                rewritten: &hints.rewritten,
            },
            (old_from, o),
            (new_from, n),
            &mut matched,
        );
        if n < new.len() {
            matched[n] = Some(o);
        }
        old_from = o + 1;
        new_from = n + 1;
    }
    pair_moves(&old, &new, &mut matched);
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

fn tokens(text: &str) -> HashSet<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}

fn similar(a: &ProjectionNode, b: &ProjectionNode) -> bool {
    if discriminant(&a.kind) != discriminant(&b.kind) {
        return false;
    }
    match (a.text.as_deref(), b.text.as_deref()) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let (a, b) = (tokens(a), tokens(b));
            let union = a.union(&b).count();
            if union == 0 {
                return true;
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "token counts are far below f64's exact integer range"
            )]
            let score = a.intersection(&b).count() as f64 / union as f64;
            score >= SIMILARITY
        },
        _ => false,
    }
}

fn pair_moves(old: &[&ProjectionNode], new: &[&ProjectionNode], matched: &mut [Option<usize>]) {
    let used: HashSet<usize> = matched.iter().flatten().copied().collect();
    let mut free: HashMap<&str, Vec<usize>> = HashMap::new();
    for (o, node) in old.iter().enumerate().rev() {
        if !used.contains(&o) {
            free.entry(node.hash8.as_str()).or_default().push(o);
        }
    }
    for (n, node) in new.iter().enumerate() {
        if matched[n].is_some() {
            continue;
        }
        let Some(candidates) = free.get_mut(node.hash8.as_str()) else {
            continue;
        };
        if let Some(at) = candidates.iter().rposition(|&o| same_key(old[o], node)) {
            matched[n] = Some(candidates.remove(at));
        }
    }
}

#[derive(Clone, Copy)]
struct Gap<'a, 'p> {
    old: &'a [&'p ProjectionNode],
    new: &'a [&'p ProjectionNode],
    rewritten: &'a HashSet<DocumentNodeId>,
}

impl Gap<'_, '_> {
    fn pairs(&self, o: usize, n: usize) -> bool {
        let (old, new) = (self.old[o], self.new[n]);
        similar(old, new)
            || (self.rewritten.contains(&old.id)
                && discriminant(&old.kind) == discriminant(&new.kind))
    }
}

fn pair_gap(
    gap: Gap<'_, '_>,
    (old_from, old_to): (usize, usize),
    (new_from, new_to): (usize, usize),
    matched: &mut [Option<usize>],
) {
    let old_len = old_to.saturating_sub(old_from);
    let new_len = new_to.saturating_sub(new_from);
    if old_len == 1
        && new_len == 1
        && discriminant(&gap.old[old_from].kind) == discriminant(&gap.new[new_from].kind)
    {
        matched[new_from] = Some(old_from);
        return;
    }
    let mut front = 0;
    while front < old_len.min(new_len) && gap.pairs(old_from + front, new_from + front) {
        matched[new_from + front] = Some(old_from + front);
        front += 1;
    }
    let mut back = 0;
    while front + back < old_len.min(new_len) && gap.pairs(old_to - 1 - back, new_to - 1 - back) {
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
