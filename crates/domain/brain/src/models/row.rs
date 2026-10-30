//! Raw fact and revision rows and their decoding into the shared
//! `BrainFact` and `FactRevision` values; a row the store cannot decode is
//! an `InvalidData` error, never a guessed default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use systemprompt_identifiers::{BrainFactId, BrainFactRevisionId, FactKindId, TopicId, UserId};
use systemprompt_models::brain::{
    BrainFact, FactRevision, FactScope, FactSource, FactStatus, RevisionChange, ScopeLevel,
    SubjectKey,
};
use systemprompt_traits::RepositoryError;

// JSON: fact fields are stored as a JSONB object.
pub(crate) type FieldMap = serde_json::Map<String, serde_json::Value>;

pub(crate) struct FactRow {
    pub id: BrainFactId,
    pub kind: FactKindId,
    pub subject_kind: String,
    pub subject_id: String,
    pub scope_level: String,
    pub scope_key: Option<String>,
    pub statement: String,
    pub fields: Json<FieldMap>,
    pub source: Json<FactSource>,
    pub topics: Vec<String>,
    pub status: String,
    pub confirmed_by: Option<UserId>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub importance_base: f64,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub review_after: Option<DateTime<Utc>>,
    pub retention_until: Option<DateTime<Utc>>,
    pub legal_hold: bool,
    pub identity_hash: String,
    pub statement_hash: String,
    pub revision: i32,
    pub pinned: bool,
    pub owner_user_id: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<FactRow> for BrainFact {
    type Error = RepositoryError;

    fn try_from(row: FactRow) -> Result<Self, Self::Error> {
        Ok(Self {
            scope: decode_scope(&row.scope_level, row.scope_key)?,
            status: decode_status(&row.status)?,
            topics: decode_topics(row.topics)?,
            id: row.id,
            kind: row.kind,
            subject: SubjectKey::new(row.subject_kind, row.subject_id),
            statement: row.statement,
            fields: row.fields.0,
            source: row.source.0,
            confirmed_by: row.confirmed_by,
            confirmed_at: row.confirmed_at,
            importance_base: row.importance_base,
            valid_from: row.valid_from,
            valid_to: row.valid_to,
            review_after: row.review_after,
            retention_until: row.retention_until,
            legal_hold: row.legal_hold,
            identity_hash: row.identity_hash,
            statement_hash: row.statement_hash,
            revision: row.revision,
            pinned: row.pinned,
            owner_user_id: row.owner_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

pub(crate) struct RevisionRow {
    pub id: BrainFactRevisionId,
    pub fact_id: BrainFactId,
    pub revision: i32,
    pub change: String,
    pub actor_user_id: Option<UserId>,
    pub status: String,
    pub statement: String,
    pub fields: Json<FieldMap>,
    pub recorded_at: DateTime<Utc>,
}

impl TryFrom<RevisionRow> for FactRevision {
    type Error = RepositoryError;

    fn try_from(row: RevisionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            change: decode_change(&row.change)?,
            status: decode_status(&row.status)?,
            id: row.id,
            fact_id: row.fact_id,
            revision: row.revision,
            actor: row.actor_user_id,
            statement: row.statement,
            fields: row.fields.0,
            recorded_at: row.recorded_at,
        })
    }
}

pub(crate) fn decode_status(value: &str) -> Result<FactStatus, RepositoryError> {
    value
        .parse()
        .map_err(|_| RepositoryError::invalid_data("status", format!("unknown status {value:?}")))
}

fn decode_change(value: &str) -> Result<RevisionChange, RepositoryError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .map_err(|_| RepositoryError::invalid_data("change", format!("unknown change {value:?}")))
}

fn decode_scope(level: &str, key: Option<String>) -> Result<FactScope, RepositoryError> {
    let level: ScopeLevel = level
        .parse()
        .map_err(|_| RepositoryError::invalid_data("scope_level", format!("unknown {level:?}")))?;
    let missing = || RepositoryError::invalid_data("scope_key", "missing for a keyed scope");
    Ok(match level {
        ScopeLevel::User => FactScope::User(UserId::new(key.ok_or_else(missing)?)),
        ScopeLevel::Project => FactScope::Project(key.ok_or_else(missing)?),
        ScopeLevel::Group => FactScope::Group(key.ok_or_else(missing)?),
        ScopeLevel::Org => FactScope::Org,
    })
}

fn decode_topics(topics: Vec<String>) -> Result<Vec<TopicId>, RepositoryError> {
    topics
        .into_iter()
        .map(|topic| TopicId::try_new(topic).map_err(|e| RepositoryError::decode("topics", e)))
        .collect()
}
