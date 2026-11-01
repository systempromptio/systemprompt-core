use chrono::{DateTime, TimeZone, Utc};
use systemprompt_brain::NewFact;
use systemprompt_identifiers::{
    BrainFactId, DocumentId, DocumentRevisionId, FactKindId, TopicId, UserId,
};
use systemprompt_models::brain::{FactCandidate, FactScope, FactSource, FactStatus, SubjectKey};

pub(crate) fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).single().expect("timestamp")
}

pub(crate) fn user(value: &str) -> UserId {
    UserId::new(value)
}

pub(crate) fn kind(value: &str) -> FactKindId {
    FactKindId::try_new(value).expect("kind")
}

pub(crate) fn topic(value: &str) -> TopicId {
    TopicId::try_new(value).expect("topic")
}

pub(crate) fn fact_id() -> BrainFactId {
    BrainFactId::generate()
}

pub(crate) fn subject(value: &str) -> SubjectKey {
    value.parse().expect("subject")
}

pub(crate) fn document_source(document: &str, revision: &str) -> FactSource {
    FactSource::Document {
        document_id: DocumentId::new(document),
        revision_id: DocumentRevisionId::new(revision),
        span: None,
    }
}

pub(crate) fn candidate(kind_id: &str, subject_key: &str) -> FactCandidate {
    FactCandidate::builder(
        kind(kind_id),
        subject(subject_key),
        document_source("doc-1", "rev-1"),
    )
    .field("owner", serde_json::json!("billing"))
    .topic(topic("ops"))
    .extractor("test")
    .build()
}

pub(crate) fn new_fact(kind_id: &str, subject_key: &str, statement: &str) -> NewFact {
    NewFact::builder(
        candidate(kind_id, subject_key),
        FactScope::Org,
        statement,
        format!("identity:{kind_id}:{subject_key}"),
    )
    .status(FactStatus::Proposed)
    .importance_base(0.5)
    .build()
}
