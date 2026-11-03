//! The retrieval tiers' SQL.
//!
//! Every tier runs over the principal's **candidate set**: the facts
//! `brain_working_set` indexes under a scope key the principal holds
//! (`RecallCtx::held_keys`), joined to `brain_facts` with the same status,
//! review and validity predicates. A fact is admitted as of
//! `RecallCtx::as_of`: observed by then, valid then and not yet due for
//! review then; confirmed, proposed only for its owner, stale only when
//! the profile injects stale facts with a marker. Each tier returns fact
//! ids ordered by its rank, then stored importance, then id, so two
//! identical recalls answer identically. The subject and linked tiers
//! name at most `max_subjects` subjects whose alias similarity reaches
//! `alias_min_sim` (recall plan P2 step 6); the full-text tier runs a
//! second plan at `retry_quorum` when the first returns nothing (step 5).
//!
//! `brain.recall.working_set: false` runs the previous scope-predicate SQL
//! in the legacy module instead; that path is removed in the next release.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod fulltext;
mod legacy;
mod subject;
mod working_set;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::{RecallQuery, SynonymsDoc, WithheldReason};
use systemprompt_traits::RepositoryError;

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
    pub working_set: bool,
    pub links_weight: f64,
    pub alias_min_sim: f64,
    pub max_subjects: i64,
}

impl TierFilter {
    #[must_use]
    pub fn with_subjects(mut self, query: &RecallQuery) -> Self {
        self.alias_min_sim = query.alias_min_sim;
        self.max_subjects = i64::from(query.max_subjects.max(1));
        self
    }
}

pub async fn pinned(pool: &PgPool, f: &TierFilter) -> Result<Vec<BrainFactId>, RepositoryError> {
    if f.working_set {
        working_set::pinned(pool, f).await
    } else {
        legacy::pinned(pool, f).await
    }
}

pub async fn by_subject(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    if hints.is_empty() {
        return Ok(Vec::new());
    }
    if f.working_set {
        subject::by_subject(pool, f, hints).await
    } else {
        legacy::by_subject(pool, f, hints).await
    }
}

pub async fn by_linked(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    if hints.is_empty() || !f.working_set {
        return Ok(Vec::new());
    }
    subject::by_linked(pool, f, hints).await
}

pub async fn by_context(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    if f.working_set {
        working_set::by_context(pool, f).await
    } else {
        legacy::by_context(pool, f).await
    }
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
    if !f.working_set {
        let query = super::synonyms::search_query(input.hints, input.synonyms);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        return legacy::full_text(pool, f, &query).await;
    }
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
    if f.working_set {
        working_set::withheld_counts(pool, f).await
    } else {
        Ok(BTreeMap::new())
    }
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
