//! What a message says about itself before any fact is read: the kinds
//! it asks, the names it carries, and the two rules built on them.
//!
//! The signal is read from the core tiers only: an extension retriever's
//! rank is never a signal. A message *asks* a kind when one of the kind's ask
//! words (`KindDecl::ask_words`) is among its lower-cased alphanumeric tokens.
//! A *name* is a capitalised word of three or more letters that does not
//! open a sentence (the first token, or one after a token ending in
//! `.`, `?` or `!`) and is not a declared stop word; it is returned
//! lower-cased, once. The unknown-name rule
//! ([`withhold_unknown_names`]) applies when the retrieval declares it,
//! no subject matched, and the store knows none of the message's names:
//! the full-text tier's ids are emptied ([`withhold_full_text`], which
//! returns how many of them no other tier returned), that count is
//! recorded as `unknown_name`, and the rule reports that it fired so an
//! asked kind cannot open the context cap for a name the brain does not
//! know. The signal itself is read from the facts the reader can see:
//! a subject whose every fact is hidden from the reader names nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::{FactRecallSource, RecallError, TierIds, WithheldSummary};
use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{
    KindDecl, RetrievalDoc, RetrievalTier, ScoredFact, WithheldReason,
};

use super::budget::RecallSignal;
use super::fusion::found_by;

const MIN_NAME_CHARS: usize = 3;

#[must_use]
pub fn tokens(hints: &str) -> Vec<String> {
    hints
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[must_use]
pub fn asked_kinds(hints: &str, kinds: &[KindDecl]) -> Vec<FactKindId> {
    let tokens = tokens(hints);
    kinds
        .iter()
        .filter(|kind| kind.asked_by(&tokens.iter().map(String::as_str).collect::<Vec<_>>()))
        .map(|kind| kind.kind.clone())
        .collect()
}

#[must_use]
pub fn names(hints: &str, stop_words: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut initial = true;
    for raw in hints.split_whitespace() {
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if is_name(word) {
            let lower = word.to_lowercase();
            if !stop_words.contains(&lower) && !initial && !names.contains(&lower) {
                names.push(lower);
            }
        }
        initial = raw.ends_with(['.', '?', '!']);
    }
    names
}

fn is_name(word: &str) -> bool {
    word.chars().count() >= MIN_NAME_CHARS
        && word.chars().all(char::is_alphabetic)
        && word.chars().next().is_some_and(char::is_uppercase)
}

#[must_use]
pub fn signal_of(fused: &[ScoredFact], kind_asked: bool) -> RecallSignal {
    let any = |tier: RetrievalTier| fused.iter().any(|scored| found_by(scored, tier));
    RecallSignal {
        subject_named: any(RetrievalTier::Subject) || any(RetrievalTier::Linked),
        full_text_hit: any(RetrievalTier::FullText),
        kind_asked,
    }
}

#[must_use]
pub fn subject_matched(tiers: &[TierIds]) -> bool {
    tiers.iter().any(|hits| {
        matches!(hits.tier, RetrievalTier::Subject | RetrievalTier::Linked) && !hits.ids.is_empty()
    })
}

#[must_use]
pub fn withhold_full_text(tiers: &mut [TierIds]) -> u32 {
    let elsewhere: Vec<_> = tiers
        .iter()
        .filter(|hits| hits.tier != RetrievalTier::FullText)
        .flat_map(|hits| hits.ids.iter().cloned())
        .collect();
    let mut dropped = 0;
    for hits in tiers
        .iter_mut()
        .filter(|h| h.tier == RetrievalTier::FullText)
    {
        dropped += hits.ids.iter().filter(|id| !elsewhere.contains(id)).count();
        hits.ids.clear();
    }
    u32::try_from(dropped).unwrap_or(u32::MAX)
}

pub async fn withhold_unknown_names(
    store: &dyn FactRecallSource,
    hints: &str,
    retrieval: &RetrievalDoc,
    tiers: &mut [TierIds],
    withheld: &mut WithheldSummary,
) -> Result<bool, RecallError> {
    if !retrieval.unknown_name_withholds_full_text || subject_matched(tiers) {
        return Ok(false);
    }
    let names = names(hints, &retrieval.search_stop_words);
    if names.is_empty() || store.unknown_terms(&names).await?.is_empty() {
        return Ok(false);
    }
    let dropped = withhold_full_text(tiers);
    if dropped > 0 {
        *withheld
            .by_reason
            .entry(WithheldReason::UnknownName)
            .or_default() += dropped;
    }
    Ok(true)
}
