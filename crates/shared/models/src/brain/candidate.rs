//! A fact an extractor proposes, before validation, dedupe and routing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{FactKindId, TopicId};

use super::scope::FactScope;
use super::source::FactSource;
use super::subject::SubjectKey;

/// An unvalidated fact. `scope` is `None` when routing decides it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactCandidate {
    pub kind: FactKindId,
    pub subject: SubjectKey,
    // JSON: candidate fields arrive from extractor output and are checked against the kind.
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub source: FactSource,
    pub topics: Vec<TopicId>,
    pub scope: Option<FactScope>,
    pub extractor: String,
    pub confidence: Option<f32>,
}

impl FactCandidate {
    #[must_use]
    pub fn builder(
        kind: FactKindId,
        subject: SubjectKey,
        source: FactSource,
    ) -> FactCandidateBuilder {
        FactCandidateBuilder {
            candidate: Self {
                kind,
                subject,
                fields: serde_json::Map::new(),
                source,
                topics: Vec::new(),
                scope: None,
                extractor: String::new(),
                confidence: None,
            },
        }
    }
}

/// Builds a [`FactCandidate`] from its kind, subject and source.
#[derive(Debug, Clone)]
pub struct FactCandidateBuilder {
    candidate: FactCandidate,
}

impl FactCandidateBuilder {
    #[must_use]
    // JSON: candidate field values are free-form until validated against the kind.
    pub fn field(mut self, name: impl Into<String>, value: serde_json::Value) -> Self {
        self.candidate.fields.insert(name.into(), value);
        self
    }

    #[must_use]
    // JSON: candidate fields arrive from extractor output and are checked against
    // the kind.
    pub fn fields(mut self, fields: serde_json::Map<String, serde_json::Value>) -> Self {
        self.candidate.fields = fields;
        self
    }

    #[must_use]
    pub fn topic(mut self, topic: TopicId) -> Self {
        self.candidate.topics.push(topic);
        self
    }

    #[must_use]
    pub fn scope(mut self, scope: FactScope) -> Self {
        self.candidate.scope = Some(scope);
        self
    }

    #[must_use]
    pub fn extractor(mut self, extractor: impl Into<String>) -> Self {
        self.candidate.extractor = extractor.into();
        self
    }

    #[must_use]
    pub const fn confidence(mut self, confidence: f32) -> Self {
        self.candidate.confidence = Some(confidence);
        self
    }

    #[must_use]
    pub fn build(self) -> FactCandidate {
        self.candidate
    }
}
