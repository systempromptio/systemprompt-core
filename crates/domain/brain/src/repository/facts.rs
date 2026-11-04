//! [`FactRepository`]: reading facts by id, identity, subject and review
//! state, and erasing them. Inserting a candidate lives in `fact_insert`
//! and every change to a stored fact goes through [`FactRepository::apply`].
//! An erase leaves the derived indexes to the `ON DELETE CASCADE` of
//! `brain_working_set` and `brain_fact_links`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{BrainFactId, FactKindId, UserId};
use systemprompt_models::brain::{BrainFact, FactSource, FactStatus};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::tombstones::TombstoneRepository;
use crate::index::release_alias_owners;
use crate::models::{FactFilter, FactRow, FieldMap, decode_status};

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
                      importance_base, observed_at, valid_from, valid_to, review_after,
                      retention_until,
                      legal_hold, identity_hash, statement_hash, revision, pinned,
                      owner_user_id AS "owner_user_id: UserId", usage_cited_rate,
                      usage_ignored_rate, created_at, updated_at
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

    pub(super) async fn id_by_identity(
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
                 AND ($6::timestamptz IS NULL OR (
                     observed_at <= $6
                     AND (valid_from IS NULL OR valid_from <= $6)
                     AND (valid_to IS NULL OR valid_to > $6)))
               ORDER BY updated_at DESC, id LIMIT $5"#,
            filter.kind.as_ref().map(FactKindId::as_str),
            filter.subject.as_ref().map(|s| s.kind.as_str()),
            filter.subject.as_ref().map(|s| s.id.as_str()),
            &statuses,
            filter.limit,
            filter.as_of,
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
        TombstoneRepository::insert(&mut *conn, &fact.kind, &fact.identity_hash, erased_by).await?;
        release_alias_owners(conn, std::slice::from_ref(id)).await?;
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
