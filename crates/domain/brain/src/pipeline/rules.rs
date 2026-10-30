//! The rules stage.
//!
//! Deterministic enrichment of a candidate before it is validated. A kind's
//! declared topics are added, the subject is resolved through its aliases,
//! and an interest matches a stored fact by its declared kinds, topics,
//! subjects, keywords and pathways.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_models::brain::{FactCandidate, InterestDecl, KindDecl, Pathway, SubjectKey};

use crate::error::BrainResult;
use crate::repository::AliasRepository;

pub const ALIAS_THRESHOLD: f32 = 0.9;

/// What an interest is matched against: a candidate about to be stored,
/// its rendered statement and the pathway it arrived by.
#[derive(Debug, Clone, Copy)]
pub struct InterestProbe<'a> {
    pub candidate: &'a FactCandidate,
    pub statement: &'a str,
    pub pathway: Pathway,
    pub now: DateTime<Utc>,
}

pub fn add_kind_topics(candidate: &mut FactCandidate, kind: &KindDecl) {
    for topic in &kind.topics {
        if !candidate.topics.contains(topic) {
            candidate.topics.push(topic.clone());
        }
    }
}

pub async fn resolve_alias(
    conn: &mut PgConnection,
    subject: &SubjectKey,
) -> BrainResult<SubjectKey> {
    let matches =
        AliasRepository::similar(conn, &subject.kind, &subject.id, ALIAS_THRESHOLD).await?;
    Ok(matches
        .into_iter()
        .next()
        .map_or_else(|| subject.clone(), |found| found.subject))
}

#[must_use]
pub fn matching_interests<'a>(
    interests: &'a [InterestDecl],
    probe: &InterestProbe<'_>,
) -> Vec<&'a InterestDecl> {
    interests
        .iter()
        .filter(|interest| interest_matches(interest, probe))
        .collect()
}

#[must_use]
pub fn interest_matches(interest: &InterestDecl, probe: &InterestProbe<'_>) -> bool {
    if interest.expires.is_some_and(|expires| expires <= probe.now) {
        return false;
    }
    let rule = &interest.r#match;
    let candidate = probe.candidate;
    let subject = candidate.subject.to_string();
    let statement = probe.statement.to_lowercase();
    (interest.pathways.is_empty() || interest.pathways.contains(&probe.pathway))
        && (rule.kinds.is_empty() || rule.kinds.contains(&candidate.kind))
        && (rule.topics.is_empty() || rule.topics.iter().any(|t| candidate.topics.contains(t)))
        && (rule.subjects.is_empty()
            || rule
                .subjects
                .iter()
                .any(|s| *s == subject || *s == candidate.subject.id))
        && (rule.keywords.is_empty()
            || rule
                .keywords
                .iter()
                .any(|k| statement.contains(&k.to_lowercase())))
}
