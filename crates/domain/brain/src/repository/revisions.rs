//! [`FactRevisionRepository`]: the append-only revision history. Rows are
//! inserted by [`super::FactRepository`] alongside the change they record
//! and never updated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::types::Json;
use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{BrainFactId, BrainFactRevisionId, UserId};
use systemprompt_models::brain::FactRevision;
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{FieldMap, NewRevision, RevisionRow};

#[derive(Debug, Clone)]
pub struct FactRevisionRepository {
    pools: Pools,
}

impl FactRevisionRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub(super) async fn insert(
        conn: &mut PgConnection,
        revision: &NewRevision,
    ) -> Result<FactRevision, RepositoryError> {
        let id = BrainFactRevisionId::generate();
        let row = sqlx::query_as!(
            RevisionRow,
            r#"INSERT INTO brain_fact_revisions
                   (id, fact_id, revision, change, actor_user_id, status, statement, fields)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id AS "id: BrainFactRevisionId", fact_id AS "fact_id: BrainFactId",
                         revision, change, actor_user_id AS "actor_user_id: UserId", status,
                         statement, fields AS "fields: Json<FieldMap>", recorded_at"#,
            id.as_str(),
            revision.fact_id.as_str(),
            revision.revision,
            revision.change.as_str(),
            revision.actor.as_ref().map(UserId::as_str),
            revision.status.as_str(),
            revision.statement,
            serde_json::Value::Object(revision.fields.clone()),
        )
        .fetch_one(conn)
        .await?;
        FactRevision::try_from(row)
    }

    pub async fn list(&self, fact_id: &BrainFactId) -> Result<Vec<FactRevision>, RepositoryError> {
        Self::list_on(&*self.pools.read, fact_id).await
    }

    pub async fn list_on(
        executor: impl PgExecutor<'_>,
        fact_id: &BrainFactId,
    ) -> Result<Vec<FactRevision>, RepositoryError> {
        sqlx::query_as!(
            RevisionRow,
            r#"SELECT id AS "id: BrainFactRevisionId", fact_id AS "fact_id: BrainFactId",
                      revision, change, actor_user_id AS "actor_user_id: UserId", status,
                      statement, fields AS "fields: Json<FieldMap>", recorded_at
               FROM brain_fact_revisions WHERE fact_id = $1 ORDER BY revision"#,
            fact_id.as_str(),
        )
        .fetch_all(executor)
        .await?
        .into_iter()
        .map(FactRevision::try_from)
        .collect()
    }
}
