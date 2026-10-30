//! [`FactRepository`]: inserting candidates, reading facts by id, identity,
//! subject and review state, and erasing them. Every change to a stored
//! fact goes through [`FactRepository::apply`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{
    BrainFactId, DocumentCommentId, DocumentId, DocumentRevisionId, FactKindId, UserId,
};
use systemprompt_models::brain::{BrainFact, FactSource, FactStatus, RevisionChange};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::fact_apply::SourceColumns;
use super::revisions::FactRevisionRepository;
use super::tombstones::TombstoneRepository;
use crate::models::{FactFilter, FactRow, FieldMap, Inserted, NewFact, NewRevision, decode_status};

/// How many facts the store holds in each status.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactCounts {
    pub by_status: Vec<(FactStatus, i64)>,
}

impl FactCounts {
    #[must_use]
    pub fn total(&self) -> i64 {
        self.by_status.iter().map(|(_, count)| count).sum()
    }
}

#[derive(Debug, Clone)]
pub struct FactRepository {
    pub(super) pools: Pools,
}

impl FactRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn insert_candidate(
        conn: &mut PgConnection,
        fact: &NewFact,
    ) -> Result<Inserted, RepositoryError> {
        let candidate = &fact.candidate;
        let columns = SourceColumns::of(&candidate.source);
        let topics: Vec<String> = candidate
            .topics
            .iter()
            .map(|t| t.as_str().to_owned())
            .collect();
        let fields = serde_json::Value::Object(candidate.fields.clone());
        let source = serde_json::to_value(&candidate.source)?;
        let created = sqlx::query_scalar!(
            r#"INSERT INTO brain_facts (
                   id, kind, subject_kind, subject_id, scope_level, scope_key, statement,
                   fields, source, source_kind, source_document_id, source_revision_id,
                   source_comment_id, topics, status, importance_base, valid_from, valid_to,
                   review_after, retention_until, identity_hash, statement_hash,
                   owner_user_id, pipeline_version)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                       $16, $17, $18, $19, $20, $21, $22, $23, $24)
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
        )
        .fetch_optional(&mut *conn)
        .await?;
        let Some(id) = created else {
            let existing = Self::id_by_identity(&mut *conn, &candidate.kind, &fact.identity_hash)
                .await?
                .ok_or_else(|| RepositoryError::not_found("brain_fact", &fact.identity_hash))?;
            return Ok(Inserted {
                id: existing,
                created: false,
            });
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
        Ok(Inserted { id, created: true })
    }

    pub async fn load(
        executor: impl PgExecutor<'_>,
        ids: &[BrainFactId],
    ) -> Result<Vec<BrainFact>, RepositoryError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        sqlx::query_as!(
            FactRow,
            r#"SELECT id AS "id: BrainFactId", kind AS "kind: FactKindId", subject_kind,
                      subject_id, scope_level, scope_key, statement,
                      fields AS "fields: Json<FieldMap>", source AS "source: Json<FactSource>",
                      topics, status, confirmed_by AS "confirmed_by: UserId", confirmed_at,
                      importance_base, valid_from, valid_to, review_after, retention_until,
                      legal_hold, identity_hash, statement_hash, revision, pinned,
                      owner_user_id AS "owner_user_id: UserId", created_at, updated_at
               FROM brain_facts WHERE id = ANY($1) ORDER BY id"#,
            &ids,
        )
        .fetch_all(executor)
        .await?
        .into_iter()
        .map(BrainFact::try_from)
        .collect()
    }

    pub async fn find(&self, id: &BrainFactId) -> Result<Option<BrainFact>, RepositoryError> {
        Self::find_on(&*self.pools.read, id).await
    }

    pub async fn find_on(
        executor: impl PgExecutor<'_>,
        id: &BrainFactId,
    ) -> Result<Option<BrainFact>, RepositoryError> {
        Ok(Self::load(executor, std::slice::from_ref(id)).await?.pop())
    }

    pub async fn by_identity(
        conn: &mut PgConnection,
        kind: &FactKindId,
        identity_hash: &str,
    ) -> Result<Option<BrainFact>, RepositoryError> {
        match Self::id_by_identity(&mut *conn, kind, identity_hash).await? {
            Some(id) => Self::find_on(conn, &id).await,
            None => Ok(None),
        }
    }

    async fn id_by_identity(
        executor: impl PgExecutor<'_>,
        kind: &FactKindId,
        identity_hash: &str,
    ) -> Result<Option<BrainFactId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainFactId" FROM brain_facts
               WHERE kind = $1 AND identity_hash = $2"#,
            kind.as_str(),
            identity_hash,
        )
        .fetch_optional(executor)
        .await?)
    }

    pub async fn by_subject_kind(
        conn: &mut PgConnection,
        filter: &FactFilter,
    ) -> Result<Vec<BrainFact>, RepositoryError> {
        let statuses: Vec<String> = filter
            .statuses
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect();
        let ids = sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainFactId" FROM brain_facts
               WHERE ($1::text IS NULL OR kind = $1)
                 AND ($2::text IS NULL OR subject_kind = $2)
                 AND ($3::text IS NULL OR subject_id = $3)
                 AND (cardinality($4::text[]) = 0 OR status = ANY($4))
               ORDER BY updated_at DESC, id LIMIT $5"#,
            filter.kind.as_ref().map(FactKindId::as_str),
            filter.subject.as_ref().map(|s| s.kind.as_str()),
            filter.subject.as_ref().map(|s| s.id.as_str()),
            &statuses,
            filter.limit,
        )
        .fetch_all(&mut *conn)
        .await?;
        Self::load(conn, &ids).await
    }

    pub async fn list(&self, filter: &FactFilter) -> Result<Vec<BrainFact>, RepositoryError> {
        let mut conn = self.pools.read.acquire().await?;
        Self::by_subject_kind(&mut conn, filter).await
    }

    pub async fn list_review_due(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<BrainFact>, RepositoryError> {
        let ids = sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainFactId" FROM brain_facts
               WHERE status IN ('confirmed', 'proposed') AND review_after < $1
                 AND (review_notified_at IS NULL OR review_notified_at < review_after)
               ORDER BY review_after, id LIMIT $2"#,
            now,
            limit,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        Self::load(&*self.pools.read, &ids).await
    }

    pub async fn erase(
        conn: &mut PgConnection,
        id: &BrainFactId,
        erased_by: Option<&UserId>,
    ) -> Result<Option<BrainFact>, RepositoryError> {
        let Some(fact) = Self::find_on(&mut *conn, id).await? else {
            return Ok(None);
        };
        sqlx::query!("DELETE FROM brain_facts WHERE id = $1", id.as_str())
            .execute(&mut *conn)
            .await?;
        TombstoneRepository::insert(conn, &fact.kind, &fact.identity_hash, erased_by).await?;
        Ok(Some(fact))
    }

    pub async fn counts(&self) -> Result<FactCounts, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT status, count(*) AS "count!" FROM brain_facts GROUP BY status ORDER BY status"#
        )
        .fetch_all(&*self.pools.read)
        .await?;
        let by_status = rows
            .into_iter()
            .map(|row| Ok((decode_status(&row.status)?, row.count)))
            .collect::<Result<_, RepositoryError>>()?;
        Ok(FactCounts { by_status })
    }
}
