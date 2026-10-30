//! Cross-reference validation of a [`BrainDocs`] set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::{FactKindId, TopicId};

use super::{BrainDocs, ImportanceConfig, RetrievalDoc};
use crate::brain::kind_decl::{ExtractorDecl, KindDecl};
use crate::brain::statement::{StatementError, placeholders};

/// A declaration set that is internally inconsistent.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BrainDeclError {
    #[error("{what} {id} is declared more than once")]
    Duplicate { what: &'static str, id: String },
    #[error("{owner} references undeclared topic {topic}")]
    UnknownTopic { owner: String, topic: TopicId },
    #[error("{owner} references undeclared kind {kind}")]
    UnknownKind { owner: String, kind: FactKindId },
    #[error("kind {kind} has an invalid statement template")]
    Statement {
        kind: FactKindId,
        #[source]
        source: StatementError,
    },
    #[error("kind {kind} statement names unknown field {name}")]
    UnknownPlaceholder { kind: FactKindId, name: String },
    #[error("kind {0} declares no identity")]
    EmptyIdentity(FactKindId),
    #[error("kind {kind} identity names unknown field {name}")]
    UnknownIdentityField { kind: FactKindId, name: String },
    #[error("kind {kind} materiality names unknown field {name}")]
    UnknownMaterialityField { kind: FactKindId, name: String },
    #[error("kind {0} has a model extractor without a prompt_version")]
    MissingPromptVersion(FactKindId),
    #[error("weight {name} is {value}, outside 0.0..=1.0")]
    WeightOutOfRange { name: String, value: f64 },
    #[error("recency half_life_days must be positive")]
    ZeroHalfLife,
}

pub fn validate(docs: &BrainDocs) -> Result<(), BrainDeclError> {
    let mut topics = HashSet::new();
    for topic in &docs.topics.topics {
        if !topics.insert(&topic.id) {
            return Err(duplicate("topic", topic.id.as_str()));
        }
    }
    let mut kinds = HashSet::new();
    for kind in &docs.kinds {
        if !kinds.insert(&kind.kind) {
            return Err(duplicate("kind", kind.kind.as_str()));
        }
        validate_kind(kind, &topics)?;
    }
    let mut interests = HashSet::new();
    for interest in &docs.interests {
        if !interests.insert(&interest.id) {
            return Err(duplicate("interest", interest.id.as_str()));
        }
        let owner = format!("interest {}", interest.id);
        known_topics(&owner, &interest.r#match.topics, &topics)?;
        known_kinds(&owner, &interest.r#match.kinds, &kinds)?;
    }
    let mut workflows = HashSet::new();
    for workflow in &docs.workflows {
        if !workflows.insert(&workflow.id) {
            return Err(duplicate("workflow", workflow.id.as_str()));
        }
        let owner = format!("workflow {}", workflow.id);
        known_kinds(&owner, &workflow.kinds, &kinds)?;
        known_kinds(&owner, workflow.quotas.keys(), &kinds)?;
    }
    for filter in &docs.filters.filters {
        known_kinds(&format!("filter {}", filter.name), &filter.kinds, &kinds)?;
    }
    known_kinds(
        "importance",
        docs.importance.base.kind_weight.keys(),
        &kinds,
    )?;
    known_kinds("retrieval", docs.retrieval.quotas.keys(), &kinds)?;
    validate_importance(&docs.importance)?;
    validate_retrieval(&docs.retrieval)
}

fn duplicate(what: &'static str, id: &str) -> BrainDeclError {
    BrainDeclError::Duplicate {
        what,
        id: id.to_owned(),
    }
}

fn validate_kind(kind: &KindDecl, topics: &HashSet<&TopicId>) -> Result<(), BrainDeclError> {
    known_topics(&format!("kind {}", kind.kind), &kind.topics, topics)?;
    let names = placeholders(&kind.statement).map_err(|source| BrainDeclError::Statement {
        kind: kind.kind.clone(),
        source,
    })?;
    if let Some(name) = names.into_iter().find(|name| !kind.knows(name)) {
        return Err(BrainDeclError::UnknownPlaceholder {
            kind: kind.kind.clone(),
            name,
        });
    }
    if kind.identity.is_empty() {
        return Err(BrainDeclError::EmptyIdentity(kind.kind.clone()));
    }
    if let Some(name) = kind.identity.iter().find(|name| !kind.knows(name)) {
        return Err(BrainDeclError::UnknownIdentityField {
            kind: kind.kind.clone(),
            name: name.clone(),
        });
    }
    if let Some(name) = kind
        .materiality
        .require
        .iter()
        .find(|name| !kind.fields.contains_key(*name))
    {
        return Err(BrainDeclError::UnknownMaterialityField {
            kind: kind.kind.clone(),
            name: name.clone(),
        });
    }
    if let Some(ExtractorDecl::Model { prompt_version, .. }) = &kind.extractor
        && prompt_version.as_deref().is_none_or(str::is_empty)
    {
        return Err(BrainDeclError::MissingPromptVersion(kind.kind.clone()));
    }
    Ok(())
}

fn known_topics<'a>(
    owner: &str,
    named: impl IntoIterator<Item = &'a TopicId>,
    declared: &HashSet<&TopicId>,
) -> Result<(), BrainDeclError> {
    match named.into_iter().find(|topic| !declared.contains(topic)) {
        Some(topic) => Err(BrainDeclError::UnknownTopic {
            owner: owner.to_owned(),
            topic: topic.clone(),
        }),
        None => Ok(()),
    }
}

fn known_kinds<'a>(
    owner: &str,
    named: impl IntoIterator<Item = &'a FactKindId>,
    declared: &HashSet<&FactKindId>,
) -> Result<(), BrainDeclError> {
    match named.into_iter().find(|kind| !declared.contains(kind)) {
        Some(kind) => Err(BrainDeclError::UnknownKind {
            owner: owner.to_owned(),
            kind: kind.clone(),
        }),
        None => Ok(()),
    }
}

fn weight(name: impl Into<String>, value: f64) -> Result<(), BrainDeclError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        return Ok(());
    }
    Err(BrainDeclError::WeightOutOfRange {
        name: name.into(),
        value,
    })
}

fn validate_importance(config: &ImportanceConfig) -> Result<(), BrainDeclError> {
    let base = &config.base;
    for (kind, value) in &base.kind_weight {
        weight(format!("kind_weight.{kind}"), *value)?;
    }
    for (source, value) in &base.source_weight {
        weight(format!("source_weight.{}", source.as_str()), *value)?;
    }
    if let Some(hint) = &base.model_hint {
        weight("model_hint.weight", hint.weight)?;
    }
    weight("confirmed_by_owner", base.confirmed_by_owner)?;
    let ctx = &config.in_context;
    for (workflow, kinds) in &ctx.workflow_weight {
        for (kind, value) in kinds {
            weight(format!("workflow_weight.{workflow}.{kind}"), *value)?;
        }
    }
    weight("subject_match", ctx.subject_match)?;
    weight("usage.cited_rate_weight", ctx.usage.cited_rate_weight)?;
    weight("usage.ignored_penalty", ctx.usage.ignored_penalty)?;
    if ctx.recency.half_life_days == 0 {
        return Err(BrainDeclError::ZeroHalfLife);
    }
    Ok(())
}

fn validate_retrieval(doc: &RetrievalDoc) -> Result<(), BrainDeclError> {
    let tiers = &doc.tiers;
    weight("tiers.pinned", tiers.pinned)?;
    weight("tiers.subject", tiers.subject)?;
    weight("tiers.context", tiers.context)?;
    weight("tiers.full_text", tiers.full_text)?;
    weight("tiers.extension", tiers.extension)
}
