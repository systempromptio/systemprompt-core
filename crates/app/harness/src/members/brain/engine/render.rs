//! The text the brain tools answer: one line per fact, the why-view, and a
//! failure the model can act on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TopicId;
use systemprompt_models::brain::client::{FactPage, FactWhy};
use systemprompt_models::brain::{BrainFact, FactScope};

use crate::members::brain::error::BrainError;

pub(super) fn fact_line(fact: &BrainFact) -> String {
    let mut line = format!(
        "{} r{} [{} · {} · {}] {} (about {})",
        fact.id,
        fact.revision,
        fact.kind,
        fact.status,
        scope_text(&fact.scope),
        fact.statement,
        fact.subject
    );
    if fact.pinned {
        line.push_str(" · pinned");
    }
    line
}

pub(super) fn fact_detail(fact: &BrainFact) -> String {
    let mut text = fact_line(fact);
    // Why: discard-ok: unencodable fields show as empty.
    let fields = serde_json::to_string(&fact.fields).unwrap_or_default();
    text.push_str("\nfields: ");
    text.push_str(&fields);
    if !fact.topics.is_empty() {
        let topics: Vec<&str> = fact.topics.iter().map(TopicId::as_str).collect();
        text.push_str("\ntopics: ");
        text.push_str(&topics.join(", "));
    }
    text
}

pub(super) fn page_text(page: &FactPage) -> String {
    if page.facts.is_empty() {
        return "No visible fact matches.".to_owned();
    }
    let mut text = format!("{} facts:", page.facts.len());
    for fact in &page.facts {
        text.push('\n');
        text.push_str(&fact_line(fact));
    }
    if let Some(next) = &page.next {
        text.push_str("\nMore facts follow; pass after=\"");
        text.push_str(next);
        text.push_str("\" for the next page.");
    }
    text
}

pub(super) fn why_text(why: &FactWhy) -> String {
    let terms = &why.terms;
    let recall = why.withheld.map_or_else(
        || "recall would offer it".to_owned(),
        |reason| format!("recall would withhold it ({})", reason.as_str()),
    );
    format!(
        "{}\nimportance {:.3}: base {:.3}, subject {:.3}, workflow {:.3}, recency {:.3}, usage {:.3}\n{recall}",
        fact_line(&why.fact),
        terms.total,
        terms.base,
        terms.subject,
        terms.workflow,
        terms.recency,
        terms.usage,
    )
}

fn scope_text(scope: &FactScope) -> String {
    match scope.key() {
        Some(key) if !matches!(scope, FactScope::User(_)) => format!("{}:{key}", scope.level()),
        _ => scope.level().to_string(),
    }
}

pub(super) fn failure_text(error: &BrainError) -> String {
    let text = match error {
        BrainError::Disabled => "The brain is not enabled on this instance.",
        BrainError::NoInstance | BrainError::Unconfirmed => {
            "The brain is not available right now; no instance has confirmed it."
        },
        BrainError::NotFound { .. } => "No fact with that id is visible to you.",
        BrainError::Forbidden { .. } => {
            "The instance does not let you do this for that fact or scope."
        },
        BrainError::Tombstoned { .. } => "That fact was erased and cannot be recorded again.",
        BrainError::Conflict { .. } => "The instance reported a storage conflict; try again.",
        BrainError::Invalid { error_key, .. } => match error_key.as_deref() {
            Some("unknown_kind") => "That kind of fact is not declared on this instance.",
            Some("invalid_subject") => "The subject is not written as kind:id.",
            Some("invalid_fields" | "invalid_statement") => {
                "The fields do not fit the kind's declaration."
            },
            _ => "The instance refused the input.",
        },
        BrainError::InvalidInput { what } => *what,
        BrainError::Client(client) if client.is_transient() => {
            "The instance could not be reached; try again later."
        },
        BrainError::Client(_) => "The brain call failed.",
    };
    text.to_owned()
}
