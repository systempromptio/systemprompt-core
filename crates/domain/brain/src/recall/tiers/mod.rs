//! The retrieval tiers' SQL.
//!
//! Every tier runs over the principal's **candidate set**: the facts
//! `brain_working_set` indexes under a scope key the principal holds
//! (`RecallCtx::held_keys`), with the same status, review and validity
//! predicates on `brain_facts`. The set is never materialised: each tier
//! drives from its own selective index and admits a fact through a probe
//! of the working set (`brain_working_set_fact_idx`), so the cost of a
//! recall follows the hits, not the facts the principal could reach. A
//! fact is admitted as of `RecallCtx::as_of`: observed by then, valid then
//! and not yet due for review then; confirmed, proposed only for its
//! owner, stale only when the profile injects stale facts with a marker.
//! Each tier returns fact ids ordered by its rank, then stored importance,
//! then id, so two identical recalls answer identically; in the context
//! and subject tiers a fact of a kind the message asks
//! (`TierFilter::asked_kinds`) ranks before the rest. The subject and
//! linked tiers read the subjects [`matched`] resolved once per recall
//! (at most `max_subjects`, each reaching `alias_min_sim`) and run at
//! their own row limit; the linked tier walks `links_depth` hops (see
//! `linked.rs`); the full-text
//! tier runs a second plan at `retry_quorum` when the first returns
//! nothing. [`ranked`] runs the five tiers concurrently and returns their
//! ids for fusion.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod fulltext;
mod linked;
pub mod matched;
mod subject;
mod working_set;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_extension::brain::TierIds;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::{RecallQuery, RetrievalTier, SynonymsDoc, WithheldReason};
use systemprompt_traits::RepositoryError;

use self::matched::MatchedSubjects;
use super::search_plan::{self, SearchPolicy};
use crate::repository::SearchTerms;

/// The principal-side filter every tier applies.
#[derive(Debug, Clone, PartialEq)]
pub struct TierFilter {
    pub user: UserId,
    pub project: Option<String>,
    pub scopes: Vec<String>,
    pub keys: Vec<String>,
    pub include_stale: bool,
    pub as_of: DateTime<Utc>,
    pub limit: i64,
    pub links_weight: f64,
    pub alias_min_sim: f64,
    pub max_subjects: i64,
    pub asked_kinds: Vec<String>,
    pub alias_longest_wins: bool,
    pub links_depth: i32,
    pub links_decay: f64,
}

impl TierFilter {
    #[must_use]
    pub fn with_subjects(mut self, query: &RecallQuery) -> Self {
        self.alias_min_sim = query.alias_min_sim;
        self.max_subjects = i64::from(query.max_subjects.max(1));
        self.asked_kinds = query
            .asked_kinds
            .iter()
            .map(|kind| kind.as_str().to_owned())
            .collect();
        self.alias_longest_wins = query.alias_longest_wins;
        self.links_depth = i32::try_from(query.links_depth).unwrap_or(i32::MAX);
        self.links_decay = query.links_decay;
        self
    }

    #[must_use]
    pub const fn with_limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }
}

pub async fn pinned(pool: &PgPool, f: &TierFilter) -> Result<Vec<BrainFactId>, RepositoryError> {
    working_set::pinned(pool, f).await
}

pub async fn by_subject(
    pool: &PgPool,
    f: &TierFilter,
    matched: &MatchedSubjects,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    subject::by_subject(pool, f, matched).await
}

pub async fn by_linked(
    pool: &PgPool,
    f: &TierFilter,
    matched: &MatchedSubjects,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    linked::by_linked(pool, f, matched).await
}

pub async fn by_context(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    working_set::by_context(pool, f).await
}

/// What the full-text tier searches with: the message, the declared
/// synonyms, the term statistics and the planning policy.
#[derive(Debug, Clone, Copy)]
pub struct FullTextInput<'a> {
    pub hints: &'a str,
    pub synonyms: &'a SynonymsDoc,
    pub terms: &'a SearchTerms,
    pub policy: &'a SearchPolicy,
}

pub async fn full_text(
    pool: &PgPool,
    f: &TierFilter,
    input: FullTextInput<'_>,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    let Some(plan) = search_plan::plan(input.hints, input.synonyms, input.terms, input.policy)
    else {
        return Ok(Vec::new());
    };
    let found = fulltext::full_text(pool, f, &plan).await?;
    if !found.is_empty() {
        return Ok(found);
    }
    let Some(retry) = input.policy.retry() else {
        return Ok(found);
    };
    let second = search_plan::plan(input.hints, input.synonyms, input.terms, &retry)
        .filter(|second| second.filter != plan.filter);
    match second {
        Some(second) => fulltext::full_text(pool, f, &second).await,
        None => Ok(found),
    }
}

pub async fn withheld_counts(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<BTreeMap<WithheldReason, u32>, RepositoryError> {
    working_set::withheld_counts(pool, f).await
}

/// One recall's tier inputs: the filter at the tier limit, the pinned
/// tier's own limit, the subject and linked tiers' own limit, the
/// normalised hints and the full-text input.
#[derive(Debug, Clone, Copy)]
pub struct RankedInput<'a> {
    pub filter: &'a TierFilter,
    pub pinned_limit: i64,
    pub subject_limit: i64,
    pub hints: &'a str,
    pub full_text: FullTextInput<'a>,
}

pub async fn ranked(
    pool: &PgPool,
    input: RankedInput<'_>,
) -> Result<Vec<TierIds>, RepositoryError> {
    let f = input.filter;
    let matched = matched::resolve(pool, f, input.hints, input.full_text.terms).await?;
    let pinned_filter = f.clone().with_limit(input.pinned_limit);
    let subject_filter = f.clone().with_limit(input.subject_limit);
    let (pinned_ids, subject_ids, linked_ids, context_ids, text_ids) = tokio::join!(
        pinned(pool, &pinned_filter),
        by_subject(pool, &subject_filter, &matched),
        by_linked(pool, &subject_filter, &matched),
        by_context(pool, f),
        full_text(pool, f, input.full_text),
    );
    Ok(vec![
        TierIds::new(RetrievalTier::Pinned, pinned_ids?),
        TierIds::new(RetrievalTier::Subject, subject_ids?),
        TierIds::new(RetrievalTier::Linked, linked_ids?),
        TierIds::new(RetrievalTier::Context, context_ids?),
        TierIds::new(RetrievalTier::FullText, text_ids?),
    ])
}

#[must_use]
pub fn search_query(hints: &str) -> String {
    super::synonyms::search_query(hints, &SynonymsDoc::default())
}

#[must_use]
pub fn normalise_hints(hints: &str) -> String {
    hints
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
