//! `anthropic-beta` values an upstream has refused, learned per provider.
//!
//! Clients (Claude Code above all) send betas the moment they ship, and an
//! upstream that does not know one refuses the whole request with a 400
//! naming it: "Unexpected value(s) `advisor-tool-2026-03-01` for the
//! `anthropic-beta` header". A static allowlist is always one client release
//! behind, so the adapter learns instead: the named values are dropped, the
//! request is re-sent once without them, and the provider's set remembers
//! them for every later request in the process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};
use std::sync::{OnceLock, RwLock};

use systemprompt_models::wire::anthropic::ANTHROPIC_BETA_HEADER;

const REJECTION_MARKER: &str = "for the `anthropic-beta` header";

fn learned_sets() -> &'static RwLock<HashMap<String, BTreeSet<String>>> {
    static SETS: OnceLock<RwLock<HashMap<String, BTreeSet<String>>>> = OnceLock::new();
    SETS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// The beta values an upstream error message names as refused; empty when the
/// message is not a beta refusal.
#[must_use]
pub fn refused_in(message: &str) -> BTreeSet<String> {
    let Some(end) = message.find(REJECTION_MARKER) else {
        return BTreeSet::new();
    };
    message[..end]
        .split('`')
        .skip(1)
        .step_by(2)
        .flat_map(|chunk| chunk.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[must_use]
pub fn learned(provider: &str) -> BTreeSet<String> {
    learned_sets()
        .read()
        .map(|sets| sets.get(provider).cloned().unwrap_or_default())
        .unwrap_or_default()
}

pub fn learn(provider: &str, refused: &BTreeSet<String>) {
    if let Ok(mut sets) = learned_sets().write() {
        sets.entry(provider.to_owned())
            .or_default()
            .extend(refused.iter().cloned());
    }
}

/// `headers` with every value in `drop` removed from `anthropic-beta`; the
/// header itself goes when nothing is left.
#[must_use]
pub fn without(headers: Vec<(String, String)>, drop: &BTreeSet<String>) -> Vec<(String, String)> {
    if drop.is_empty() {
        return headers;
    }
    headers
        .into_iter()
        .filter_map(|(name, value)| {
            if !name.eq_ignore_ascii_case(ANTHROPIC_BETA_HEADER) {
                return Some((name, value));
            }
            let kept: Vec<&str> = value
                .split(',')
                .map(str::trim)
                .filter(|beta| !beta.is_empty() && !drop.contains(*beta))
                .collect();
            (!kept.is_empty()).then(|| (name, kept.join(",")))
        })
        .collect()
}

#[must_use]
pub fn carries_any(headers: &[(String, String)], values: &BTreeSet<String>) -> bool {
    headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case(ANTHROPIC_BETA_HEADER)
            && value.split(',').any(|beta| values.contains(beta.trim()))
    })
}
