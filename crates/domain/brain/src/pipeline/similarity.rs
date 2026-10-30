//! Trigram similarity of two statements, computed the way Postgres
//! `pg_trgm` computes `similarity()`.
//!
//! Each lowercased alphanumeric word is padded with two leading spaces and
//! one trailing space, and the score is the shared trigrams over the union.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

#[must_use]
pub fn trigram_similarity(left: &str, right: &str) -> f64 {
    let a = trigrams(left);
    let b = trigrams(right);
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let shared = a.intersection(&b).count();
    let union = a.union(&b).count();
    shared as f64 / union as f64
}

fn trigrams(text: &str) -> BTreeSet<[char; 3]> {
    let mut set = BTreeSet::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let padded: Vec<char> = "  "
            .chars()
            .chain(word.to_lowercase().chars())
            .chain(std::iter::once(' '))
            .collect();
        for window in padded.windows(3) {
            set.insert([window[0], window[1], window[2]]);
        }
    }
    set
}
