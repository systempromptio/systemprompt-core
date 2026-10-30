//! [`FactRepository::apply`]: the single mutator. It updates the fact's
//! projection, bumps its revision and appends the revision row on the same
//! connection, so a fact and its history never disagree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use sqlx::types::Json;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::{FactRevision, FactSource, statement_hash};
use systemprompt_traits::RepositoryError;

use super::FactRepository;
use super::revisions::FactRevisionRepository;
use crate::models::{FactChange, FieldMap, NewRevision, decode_status};

pub(super) struct SourceColumns<'a> {
    pub(super) kind: &'static str,
    pub(super) document_id: Option<&'a str>,
    pub(super) revision_id: Option<&'a str>,
    pub(super) comment_id: Option<&'a str>,
}

impl<'a> SourceColumns<'a> {
    pub(super) fn of(source: &'a FactSource) -> Self {
        let (revision_id, comment_id) = match source {
            FactSource::Document { revision_id, .. } => (Some(revision_id.as_str()), None),
            FactSource::Comment { comment_id, .. } => (None, Some(comment_id.as_str())),
            _ => (None, None),
        };
        Self {
            kind: source.class().as_str(),
            document_id: source.document_id().map(|id| id.as_str()),
            revision_id,
            comment_id,
        }
    }
}

#[derive(Default)]
struct Patch {
    statement: Option<String>,
    statement_hash: Option<String>,
    fields: Option<serde_json::Value>,
    source: Option<serde_json::Value>,
    topics: Option<Vec<String>>,
    importance_base: Option<f64>,
    review_after: Option<Option<DateTime<Utc>>>,
}

impl Patch {
    fn of(change: &FactChange) -> Result<Self, RepositoryError> {
        Ok(match change {
            FactChange::Revised {
                statement,
                fields,
                source,
                topics,
                importance_base,
                review_after,
                ..
            } => Self {
                statement_hash: Some(statement_hash(statement)),
                statement: Some(statement.clone()),
                fields: Some(serde_json::Value::Object(fields.clone())),
                source: Some(serde_json::to_value(source)?),
                topics: Some(topic_strings(topics)),
                importance_base: Some(*importance_base),
                review_after: Some(*review_after),
            },
            FactChange::Merged { topics, .. } => Self {
                topics: Some(topic_strings(topics)),
                ..Self::default()
            },
            FactChange::FieldsEdited {
                statement, fields, ..
            } => Self {
                statement_hash: Some(statement_hash(statement)),
                statement: Some(statement.clone()),
                fields: Some(serde_json::Value::Object(fields.clone())),
                ..Self::default()
            },
            _ => Self::default(),
        })
    }
}

fn topic_strings(topics: &[systemprompt_identifiers::TopicId]) -> Vec<String> {
    topics.iter().map(|t| t.as_str().to_owned()).collect()
}

impl FactRepository {
    pub async fn apply(
        conn: &mut PgConnection,
        id: &BrainFactId,
        change: FactChange,
    ) -> Result<FactRevision, RepositoryError> {
        let patch = Patch::of(&change)?;
        let source: Option<FactSource> = match &change {
            FactChange::Revised { source, .. } => Some(source.clone()),
            _ => None,
        };
        let columns = source.as_ref().map(SourceColumns::of);
        let confirmed_by = match &change {
            FactChange::Confirmed { by } => Some(by.as_str()),
            _ => None,
        };
        let row = sqlx::query!(
            r#"UPDATE brain_facts SET
                   status = COALESCE($2, status),
                   statement = COALESCE($3, statement),
                   statement_hash = COALESCE($4, statement_hash),
                   fields = COALESCE($5, fields),
                   source = COALESCE($6, source),
                   source_kind = COALESCE($7, source_kind),
                   source_document_id = CASE WHEN $6::jsonb IS NULL THEN source_document_id ELSE $8 END,
                   source_revision_id = CASE WHEN $6::jsonb IS NULL THEN source_revision_id ELSE $9 END,
                   source_comment_id = CASE WHEN $6::jsonb IS NULL THEN source_comment_id ELSE $10 END,
                   topics = COALESCE($11, topics),
                   importance_base = COALESCE($12, importance_base),
                   review_after = CASE WHEN $13 THEN $14 ELSE review_after END,
                   confirmed_by = CASE WHEN $15::text IS NULL THEN confirmed_by ELSE $15 END,
                   confirmed_at = CASE WHEN $15::text IS NULL THEN confirmed_at ELSE now() END,
                   pinned = COALESCE($16, pinned),
                   review_notified_at = CASE WHEN $17 THEN now() ELSE review_notified_at END,
                   revision = revision + 1,
                   updated_at = now()
               WHERE id = $1
               RETURNING revision, status, statement, fields AS "fields: Json<FieldMap>""#,
            id.as_str(),
            change.new_status().map(|s| s.as_str()),
            patch.statement,
            patch.statement_hash,
            patch.fields,
            patch.source,
            columns.as_ref().map(|c| c.kind),
            columns.as_ref().and_then(|c| c.document_id),
            columns.as_ref().and_then(|c| c.revision_id),
            columns.as_ref().and_then(|c| c.comment_id),
            patch.topics.as_deref(),
            patch.importance_base,
            patch.review_after.is_some(),
            patch.review_after.flatten(),
            confirmed_by,
            change.new_pinned(),
            matches!(change, FactChange::ReviewDue),
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| RepositoryError::not_found("brain_fact", id))?;
        FactRevisionRepository::insert(
            conn,
            &NewRevision {
                fact_id: id.clone(),
                revision: row.revision,
                change: change.revision_change(),
                actor: change.actor().map(UserId::clone),
                status: decode_status(&row.status)?,
                statement: row.statement,
                fields: row.fields.0,
            },
        )
        .await
    }
}
