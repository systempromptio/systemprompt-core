//! Deterministic block ids for an imported model.
//!
//! Import numbers blocks `b1`, `b2`, … in document order, so two replicas
//! seeding from the same revision agree on every id. Blocks a client adds
//! later carry the client's own ids. [`carry_ids`] keeps the live ids
//! across a re-import.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::rich_text::{BlockKind, RichText};

#[derive(Debug, Default)]
pub(crate) struct IdSeq(u64);

impl IdSeq {
    pub(crate) fn next_id(&mut self) -> String {
        self.0 += 1;
        format!("b{}", self.0)
    }
}

pub fn carry_ids(live: &RichText, next: &mut RichText) {
    let mut taken = HashSet::new();
    collect_ids(live, &mut taken);
    let mut fresh = Fresh {
        taken: &taken,
        n: 0,
    };
    carry(live, next, &mut fresh);
}

struct Fresh<'a> {
    taken: &'a HashSet<&'a str>,
    n: u64,
}

impl Fresh<'_> {
    fn next_id(&mut self) -> String {
        loop {
            self.n += 1;
            let id = format!("m{}", self.n);
            if !self.taken.contains(id.as_str()) {
                return id;
            }
        }
    }
}

fn collect_ids<'a>(model: &'a RichText, into: &mut HashSet<&'a str>) {
    for block in &model.blocks {
        into.insert(&block.id);
        if let BlockKind::Table { rows, .. } = &block.kind {
            for cell in rows.iter().flatten() {
                collect_ids(cell, into);
            }
        }
    }
}

fn carry(live: &RichText, next: &mut RichText, fresh: &mut Fresh<'_>) {
    const LOOKAHEAD: usize = 64;
    let mut cursor = 0;
    for at in 0..next.blocks.len() {
        let Some((block, rest)) = next.blocks[at..].split_first_mut() else {
            break;
        };
        let same = live
            .blocks
            .iter()
            .enumerate()
            .skip(cursor)
            .take(LOOKAHEAD)
            .find(|(_, old)| old.kind == block.kind && old.text == block.text);
        let edited = live
            .blocks
            .get(cursor)
            .filter(|old| std::mem::discriminant(&old.kind) == std::mem::discriminant(&block.kind))
            .filter(|old| {
                !rest
                    .iter()
                    .take(LOOKAHEAD)
                    .any(|later| later.kind == old.kind && later.text == old.text)
            })
            .map(|old| (cursor, old));
        let matched = same.or(edited);
        if let Some((found, old)) = matched {
            block.id.clone_from(&old.id);
            cursor = found + 1;
        } else {
            block.id = fresh.next_id();
        }
        if let BlockKind::Table { rows, .. } = &mut block.kind {
            carry_cells(matched.map(|(_, old)| &old.kind), rows, fresh);
        }
    }
}

fn carry_cells(old: Option<&BlockKind>, rows: &mut [Vec<RichText>], fresh: &mut Fresh<'_>) {
    let empty = RichText::default();
    let old_rows = match old {
        Some(BlockKind::Table { rows, .. }) => rows.as_slice(),
        _ => &[],
    };
    for (r, row) in rows.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            let before = old_rows.get(r).and_then(|row| row.get(c)).unwrap_or(&empty);
            carry(before, cell, fresh);
        }
    }
}
