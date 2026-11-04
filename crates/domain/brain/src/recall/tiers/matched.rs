//! The subjects a message names, resolved once per recall and handed to
//! the subject and linked tiers as parallel arrays.
//!
//! A subject is matched when one of its aliases, or its id, is a word
//! n-gram of the normalised hints (score `1.0`: `alias_norm` equal to a
//! run of one to [`MAX_NGRAM_WORDS`] words, taken both over the
//! whitespace-separated words with punctuation trimmed from their ends,
//! so `e-commerce hub,` yields `e-commerce hub`, and over the
//! alphanumeric tokens, so `acme's` yields `acme`; served by the btree
//! on `alias_norm` and on `lower(subject_id)`), or when an
//! alias is trigram-similar to the hints (the word similarity, as
//! `alias_norm <% hints` scores it). The similar branch is reached
//! through the trigram index by the message's *probe words*: the
//! alphanumeric tokens of three or more characters that no fact carries
//! (every such token while the term statistics are empty), each looked
//! up as `alias_norm %> word`, and only a candidate that satisfies the
//! whole-hints predicate scores. A subject is named only when its best
//! score reaches `alias_min_sim`, and the `max_subjects` best-scoring
//! subjects (ties by kind, then id) are the ones both tiers read. The
//! trigram operator keeps its own threshold
//! (`pg_trgm.word_similarity_threshold`, 0.6), so a lower
//! `alias_min_sim` admits nothing more; a higher one narrows the match.
//!
//! An alias that occurs only inside a longer word of the message
//! (`pay` in `payments`) is not an occurrence: it is named only if the
//! similarity branch reaches it, and then at the similarity, not `1.0`.
//!
//! With `TierFilter::alias_longest_wins`, a subject matched at `1.0`
//! whose every exact alias is a proper substring of another matched
//! subject's exact alias is dropped: `larchmont` inside `larchmont
//! platform` names the project, not the client. The drop runs after the
//! `max_subjects` cut, so it never admits a subject the cut left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_traits::RepositoryError;

use super::TierFilter;
use crate::repository::SearchTerms;

pub const MAX_NGRAM_WORDS: usize = 4;
const MIN_PROBE_CHARS: usize = 3;

/// The matched subjects as the tier SQL unnests them: one entry per
/// subject, kinds, ids and scores at the same index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatchedSubjects {
    pub kinds: Vec<String>,
    pub ids: Vec<String>,
    pub scores: Vec<f64>,
}

impl MatchedSubjects {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

#[must_use]
pub fn ngrams(hints: &str) -> Vec<String> {
    let trimmed: Vec<&str> = hints
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .collect();
    let tokens: Vec<&str> = hints
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let mut grams: Vec<String> = Vec::new();
    for words in [trimmed, tokens] {
        for start in 0..words.len() {
            let end = (start + MAX_NGRAM_WORDS).min(words.len());
            for stop in start + 1..=end {
                let gram = words[start..stop].join(" ");
                if !grams.contains(&gram) {
                    grams.push(gram);
                }
            }
        }
    }
    grams
}

#[must_use]
pub fn probes(hints: &str, terms: &SearchTerms) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in hints.split(|c: char| !c.is_alphanumeric()) {
        let known = !terms.is_empty() && terms.ndoc.contains_key(word);
        if word.chars().count() < MIN_PROBE_CHARS || known || words.iter().any(|w| w == word) {
            continue;
        }
        words.push(word.to_owned());
    }
    words
}

// Why: pg_trgm — gin_trgm_ops serves `=`, `%` and `%>` with the indexed
// column on the left, not `<%`, so the alias-within-hints predicate can
// only be reached through `alias_norm %> word` probes and rechecked. The
// probe join sits behind `OFFSET 0` because the planner costs `<%` as a
// cheap, selective filter and otherwise applies it in a sequential scan
// ahead of the index join.
pub async fn resolve(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
    terms: &SearchTerms,
) -> Result<MatchedSubjects, RepositoryError> {
    if hints.is_empty() {
        return Ok(MatchedSubjects::default());
    }
    let grams = ngrams(hints);
    let words = probes(hints, terms);
    let rows = sqlx::query!(
        r#"SELECT subject_kind AS "subject_kind!", subject_id AS "subject_id!",
                  max(score) AS "score!",
                  coalesce(array_agg(DISTINCT alias) FILTER (WHERE score >= 1.0), '{}')
                      AS "exact!"
           FROM (
               SELECT a.subject_kind, a.subject_id, a.alias_norm AS alias, 1.0::float8 AS score
               FROM brain_aliases a
               WHERE a.alias_norm = ANY($2::text[])
               UNION ALL
               SELECT c.subject_kind, c.subject_id, c.alias_norm,
                      word_similarity(c.alias_norm, $1)::float8
               FROM (SELECT DISTINCT a.subject_kind, a.subject_id, a.alias_norm
                     FROM unnest($3::text[]) AS w(word)
                     JOIN brain_aliases a ON a.alias_norm %> w.word
                     OFFSET 0) c
               WHERE c.alias_norm <% $1
               UNION ALL
               SELECT s.subject_kind, s.subject_id, lower(s.subject_id), 1.0::float8
               FROM brain_subjects s
               WHERE length(s.subject_id) >= 3 AND lower(s.subject_id) = ANY($2::text[])
           ) matched_hits GROUP BY subject_kind, subject_id
           HAVING max(score) >= $4
           ORDER BY max(score) DESC, subject_kind, subject_id
           LIMIT $5"#,
        hints,
        &grams,
        &words,
        f.alias_min_sim,
        f.max_subjects,
    )
    .fetch_all(pool)
    .await?;
    let matched_hits: Vec<Hit> = rows
        .into_iter()
        .map(|row| Hit {
            kind: row.subject_kind,
            id: row.subject_id,
            score: row.score,
            exact: row.exact,
        })
        .collect();
    let kept = if f.alias_longest_wins {
        longest_wins(matched_hits)
    } else {
        matched_hits
    };
    let mut matched = MatchedSubjects::default();
    for hit in kept {
        matched.kinds.push(hit.kind);
        matched.ids.push(hit.id);
        matched.scores.push(hit.score);
    }
    Ok(matched)
}

#[derive(Debug, Clone, PartialEq)]
struct Hit {
    kind: String,
    id: String,
    score: f64,
    exact: Vec<String>,
}

fn longest_wins(matched_hits: Vec<Hit>) -> Vec<Hit> {
    let exact: Vec<String> = matched_hits
        .iter()
        .flat_map(|h| h.exact.iter().cloned())
        .collect();
    let contained = |alias: &str| {
        exact
            .iter()
            .any(|other| other.len() > alias.len() && other.contains(alias))
    };
    matched_hits
        .into_iter()
        .filter(|hit| hit.exact.is_empty() || !hit.exact.iter().all(|alias| contained(alias)))
        .collect()
}
