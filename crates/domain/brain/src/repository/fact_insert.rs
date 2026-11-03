//! [`FactRepository::insert_candidate`]: a decided candidate becomes a
//! stored fact with its first revision. An insert refreshes the derived
//! indexes on the same connection unless the caller defers them to its
//! set-based round ([`Reindex::Deferred`]); a candidate whose identity is
//! already stored lands on that fact unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{
    BrainFactId, DocumentCommentId, DocumentId, DocumentRevisionId, FactKindId, UserId,
};
use systemprompt_models::brain::{FactCandidate, RevisionChange};
use systemprompt_traits::RepositoryError;

use super::FactRepository;
use super::fact_apply::SourceColumns;
use super::revisions::FactRevisionRepository;
use crate::index::{Reindex, reindex_fact};
use crate::models::{Inserted, NewFact, NewRevision};

impl FactRepository {
    async fn existing_by_identity(
        conn: &mut PgConnection,
        kind: &FactKindId,
        identity_hash: &str,
    ) -> Result<Inserted, RepositoryError> {
        let existing = Self::id_by_identity(&mut *conn, kind, identity_hash)
            .await?
            .ok_or_else(|| RepositoryError::not_found("brain_fact", identity_hash))?;
        Ok(Inserted {
            id: existing,
            created: false,
        })
    }

    pub async fn insert_candidate(
        conn: &mut PgConnection,
        fact: &NewFact,
    ) -> Result<Inserted, RepositoryError> {
        Self::insert_candidate_with(conn, fact, Reindex::Now).await
    }

    pub async fn insert_candidate_with(
        conn: &mut PgConnection,
        fact: &NewFact,
        reindex: Reindex,
    ) -> Result<Inserted, RepositoryError> {
        let candidate = &fact.candidate;
        let columns = SourceColumns::of(&candidate.source);
        let topics = topic_names(candidate);
        let fields = serde_json::Value::Object(candidate.fields.clone());
        let source = serde_json::to_value(&candidate.source)?;
        let created = sqlx::query_scalar!(
            r#"INSERT INTO brain_facts (
                   id, kind, subject_kind, subject_id, scope_level, scope_key, statement,
                   fields, source, source_kind, source_document_id, source_revision_id,
                   source_comment_id, topics, status, importance_base, valid_from, valid_to,
                   review_after, retention_until, identity_hash, statement_hash,
                   owner_user_id, pipeline_version, observed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                       $16, $17, $18, $19, $20, $21, $22, $23, $24, COALESCE($25, now()))
               ON CONFLICT (kind, identity_hash) DO NOTHING
               RETURNING id AS "id: BrainFactId""#,
            fact.id.as_str(),
            candidate.kind.as_str(),
            candidate.subject.kind,
            candidate.subject.id,
            fact.scope.level().as_str(),
            fact.scope.key(),
            fact.statement,
            fields,
            source,
            columns.kind,
            columns.document_id.map(DocumentId::as_str),
            columns.revision_id.map(DocumentRevisionId::as_str),
            columns.comment_id.map(DocumentCommentId::as_str),
            &topics,
            fact.status.as_str(),
            fact.importance_base,
            fact.valid_from,
            fact.valid_to,
            fact.review_after,
            fact.retention_until,
            fact.identity_hash,
            fact.statement_hash,
            fact.owner_user_id.as_ref().map(UserId::as_str),
            fact.pipeline_version
                .as_ref()
                .map(systemprompt_identifiers::PipelineVersion::as_str),
            fact.observed_at,
        )
        .fetch_optional(&mut *conn)
        .await?;
        let Some(id) = created else {
            return Self::existing_by_identity(conn, &candidate.kind, &fact.identity_hash).await;
        };
        FactRevisionRepository::insert(
            conn,
            &NewRevision {
                fact_id: id.clone(),
                revision: 1,
                change: RevisionChange::Created,
                actor: fact.owner_user_id.clone(),
                status: fact.status,
                statement: fact.statement.clone(),
                fields: candidate.fields.clone(),
            },
        )
        .await?;
        if reindex == Reindex::Now {
            reindex_fact(conn, &id).await?;
        }
        Ok(Inserted { id, created: true })
    }
}

fn topic_names(candidate: &FactCandidate) -> Vec<String> {
    candidate
        .topics
        .iter()
        .map(|t| t.as_str().to_owned())
        .collect()
}
