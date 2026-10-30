//! Order-preserving alignment of a deck list as read against a target list.
//!
//! The longest common subsequence of the two key sequences is kept; between
//! two kept entries, removed and added entries are paired in order as
//! changes, so an entry whose key the engine renamed (a slide the deck now
//! calls by its part name, a paragraph whose text was edited) is updated in
//! place rather than removed and re-added.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Keep { base: usize, target: usize },
    Change { base: usize, target: usize },
    Remove { base: usize },
    Add { target: usize },
}

pub(super) fn align<K: PartialEq>(base: &[K], target: &[K]) -> Vec<Step> {
    let (n, m) = (base.len(), target.len());
    let mut table = vec![0usize; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if base[i] == target[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut raw = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (0, 0);
    #[expect(
        clippy::suspicious_operation_groupings,
        reason = "an alignment compares base row i with target row j"
    )]
    while i < n || j < m {
        if i < n && j < m && base[i] == target[j] {
            raw.push(Step::Keep { base: i, target: j });
            i += 1;
            j += 1;
        } else if j < m && (i == n || table[at(i, j + 1)] >= table[at(i + 1, j)]) {
            raw.push(Step::Add { target: j });
            j += 1;
        } else {
            raw.push(Step::Remove { base: i });
            i += 1;
        }
    }
    pair_gaps(raw)
}

fn pair_gaps(raw: Vec<Step>) -> Vec<Step> {
    let mut out = Vec::with_capacity(raw.len());
    let mut removed = Vec::new();
    let mut added = Vec::new();
    let flush = |out: &mut Vec<Step>, removed: &mut Vec<usize>, added: &mut Vec<usize>| {
        let pairs = removed.len().min(added.len());
        for (base, target) in removed.iter().zip(added.iter()).take(pairs) {
            out.push(Step::Change {
                base: *base,
                target: *target,
            });
        }
        out.extend(
            removed
                .drain(..)
                .skip(pairs)
                .map(|base| Step::Remove { base }),
        );
        out.extend(
            added
                .drain(..)
                .skip(pairs)
                .map(|target| Step::Add { target }),
        );
    };
    for step in raw {
        match step {
            Step::Remove { base } => removed.push(base),
            Step::Add { target } => added.push(target),
            keep => {
                flush(&mut out, &mut removed, &mut added);
                out.push(keep);
            },
        }
    }
    flush(&mut out, &mut removed, &mut added);
    out
}

pub(super) fn pair<T>(
    base: &[T],
    target: &[T],
    id: impl Fn(&T) -> &str,
    rank: impl Fn(usize, &T) -> u64,
    fits: impl Fn(&T, &T) -> bool,
) -> Vec<Option<usize>> {
    let mut by_id = std::collections::HashMap::new();
    for (at, item) in base.iter().enumerate() {
        by_id.entry(id(item)).or_insert(at);
    }
    let mut used = vec![false; base.len()];
    let mut pairs: Vec<Option<usize>> = target
        .iter()
        .map(|item| {
            let at = by_id.get(id(item)).copied().filter(|at| !used[*at])?;
            used[at] = true;
            Some(at)
        })
        .collect();
    let mut free: Vec<usize> = (0..base.len()).filter(|at| !used[*at]).collect();
    free.sort_by_key(|at| rank(*at, &base[*at]));
    for (t, slot) in pairs.iter_mut().enumerate() {
        if slot.is_some() {
            continue;
        }
        if let Some(pos) = free.iter().position(|b| fits(&base[*b], &target[t])) {
            *slot = Some(free.remove(pos));
        }
    }
    pairs
}
