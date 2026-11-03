//! Query-time synonym expansion for the full-text tier.
//!
//! The index side appends a group's members to a fact's vector at the
//! lowest weight (`SynonymsDoc::expansion_text`); the query side does the
//! same for the prompt: each query word that belongs to a group is
//! expanded to the whole group, and the words are `or`-joined into one
//! `websearch` query, so a prompt that says "client" finds a fact indexed
//! under "customer" whichever side was declared later.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::SynonymsDoc;

const MAX_QUERY_WORDS: usize = 32;
const MIN_WORD_CHARS: usize = 3;

/// The group a lower-cased word belongs to, or just the word.
#[must_use]
pub fn expand<'a>(synonyms: &'a SynonymsDoc, word: &'a str) -> Vec<&'a str> {
    synonyms
        .groups
        .iter()
        .find(|group| group.iter().any(|w| w.eq_ignore_ascii_case(word)))
        .map_or_else(
            || vec![word],
            |group| group.iter().map(String::as_str).collect(),
        )
}

/// The `websearch_to_tsquery('simple', …)` text for the hints: distinct
/// lower-cased words of at least three characters, the first thirty-two,
/// each expanded to its synonym group, `or`-joined.
#[must_use]
pub fn search_query(hints: &str, synonyms: &SynonymsDoc) -> String {
    let mut words: Vec<String> = Vec::new();
    for word in hints.split(|c: char| !c.is_alphanumeric()) {
        let word = word.to_lowercase();
        if word.chars().count() < MIN_WORD_CHARS {
            continue;
        }
        for expanded in expand(synonyms, &word) {
            let expanded = expanded.to_lowercase();
            if !words.contains(&expanded) {
                words.push(expanded);
            }
        }
        if words.len() >= MAX_QUERY_WORDS {
            words.truncate(MAX_QUERY_WORDS);
            break;
        }
    }
    words.join(" or ")
}
