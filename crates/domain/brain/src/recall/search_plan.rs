//! The full-text tier's query plan: which of a message's words a fact
//! must carry, and which only rank it.
//!
//! Every word of three or more characters is expanded to its synonym
//! group. A group's document share is that of its most common member. A
//! group no fact carries is dropped (a novel word must not empty the
//! tier); a group above `search_max_df` is too common to narrow the
//! candidates and only ranks; of the informative groups, rarest first, a
//! `search_quorum` share (at least one) is required. The filter is the
//! conjunction of the required groups, each a disjunction of its members;
//! the rank is the disjunction of every group. Without statistics every
//! group ranks and the filter is the disjunction of all of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::SynonymsDoc;

use super::synonyms::expand;
use crate::repository::SearchTerms;

const MAX_QUERY_WORDS: usize = 32;
const MIN_WORD_CHARS: usize = 3;

/// How the planner decides what a fact must carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchPolicy {
    pub max_df: f64,
    pub quorum: f64,
}

/// `to_tsquery('simple', …)` text for the filter and the rank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlan {
    pub filter: String,
    pub rank: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Group {
    tsquery: String,
    share: f64,
}

#[must_use]
pub fn plan(
    hints: &str,
    synonyms: &SynonymsDoc,
    terms: &SearchTerms,
    policy: &SearchPolicy,
) -> Option<SearchPlan> {
    let groups = groups_of(hints, synonyms, terms);
    if groups.is_empty() {
        return None;
    }
    let rank = disjunction(&groups);
    let mut informative: Vec<&Group> = groups
        .iter()
        .filter(|g| g.share > 0.0 && g.share <= policy.max_df)
        .collect();
    if informative.is_empty() {
        return Some(SearchPlan {
            filter: rank.clone(),
            rank,
        });
    }
    informative.sort_by(|a, b| a.share.total_cmp(&b.share));
    let required = quorum_count(informative.len(), policy.quorum);
    let filter = informative
        .iter()
        .take(required)
        .map(|g| g.tsquery.clone())
        .collect::<Vec<_>>()
        .join(" & ");
    Some(SearchPlan { filter, rank })
}

fn quorum_count(informative: usize, quorum: f64) -> usize {
    // Why: lint-ok: a count of groups, at most thirty-two; the ceil is exact.
    let wanted = (informative as f64 * quorum.clamp(0.0, 1.0)).ceil() as usize;
    wanted.clamp(1, informative)
}

fn groups_of(hints: &str, synonyms: &SynonymsDoc, terms: &SearchTerms) -> Vec<Group> {
    let mut seen: Vec<String> = Vec::new();
    let mut groups = Vec::new();
    for word in hints.split(|c: char| !c.is_alphanumeric()) {
        let word = word.to_lowercase();
        if word.chars().count() < MIN_WORD_CHARS || seen.contains(&word) {
            continue;
        }
        let members: Vec<String> = expand(synonyms, &word)
            .into_iter()
            .map(str::to_lowercase)
            .collect();
        if members.iter().any(|m| seen.contains(m)) {
            continue;
        }
        seen.extend(members.iter().cloned());
        if let Some(group) = group_of(&members, terms) {
            groups.push(group);
        }
        if seen.len() >= MAX_QUERY_WORDS {
            break;
        }
    }
    groups
}

fn group_of(members: &[String], terms: &SearchTerms) -> Option<Group> {
    let mut share: f64 = 0.0;
    let mut alternatives = Vec::new();
    for member in members {
        let lexemes: Vec<&str> = member
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        if lexemes.is_empty() {
            continue;
        }
        share = lexemes.iter().map(|w| terms.share(w)).fold(share, f64::max);
        alternatives.push(lexemes.join(" <-> "));
    }
    (!alternatives.is_empty()).then(|| Group {
        tsquery: if alternatives.len() == 1 {
            alternatives.remove(0)
        } else {
            format!("({})", alternatives.join(" | "))
        },
        share,
    })
}

fn disjunction(groups: &[Group]) -> String {
    groups
        .iter()
        .map(|g| g.tsquery.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}
