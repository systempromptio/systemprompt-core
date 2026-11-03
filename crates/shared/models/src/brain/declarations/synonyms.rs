//! `synonyms.yaml`: groups of terms the search index treats as one.
//!
//! Expansion happens at index time, never at query time: when any member
//! of a group occurs in a fact's text, every member of that group is
//! appended to the text that is indexed at the lowest weight, so a query
//! for one spelling finds a fact that uses another. Matching is on whole
//! words, case-insensitively, and a multi-word member matches as a
//! contiguous phrase.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The declared synonym groups; the first member of a group is its
/// canonical spelling, which is only a convention for readers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SynonymsDoc {
    pub groups: Vec<Vec<String>>,
}

/// A synonyms document that cannot be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SynonymsError {
    #[error("synonym group {0} has fewer than two members")]
    ShortGroup(usize),
    #[error("synonym group {0} has an empty member")]
    EmptyMember(usize),
}

impl SynonymsDoc {
    pub fn validate(&self) -> Result<(), SynonymsError> {
        for (index, group) in self.groups.iter().enumerate() {
            if group.len() < 2 {
                return Err(SynonymsError::ShortGroup(index));
            }
            if group.iter().any(|member| member.trim().is_empty()) {
                return Err(SynonymsError::EmptyMember(index));
            }
        }
        Ok(())
    }

    /// Every member of every group with a member present in `text`, in
    /// declaration order, each member once.
    #[must_use]
    pub fn expansions(&self, text: &str) -> Vec<String> {
        let words = words_of(text);
        let mut out: Vec<String> = Vec::new();
        for group in &self.groups {
            let present = group.iter().any(|member| contains_phrase(&words, member));
            if !present {
                continue;
            }
            for member in group {
                if !out.iter().any(|seen| seen == member) {
                    out.push(member.clone());
                }
            }
        }
        out
    }

    /// The expansions of `text` joined by single spaces, ready to append
    /// to the indexed text.
    #[must_use]
    pub fn expansion_text(&self, text: &str) -> String {
        self.expansions(text).join(" ")
    }
}

fn words_of(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn contains_phrase(words: &[String], phrase: &str) -> bool {
    let needle = words_of(phrase);
    if needle.is_empty() || needle.len() > words.len() {
        return false;
    }
    words.windows(needle.len()).any(|window| window == needle)
}
