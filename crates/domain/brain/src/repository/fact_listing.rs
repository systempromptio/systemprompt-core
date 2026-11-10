//! The visible listing: facts reachable through one of the caller's
//! working-set keys, newest first, resumed past a keyset cursor. The scope
//! predicate runs in SQL before the limit, so a page is never spent on
//! facts the caller cannot reach; recall's per-fact check still runs on
//! each page.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_models::brain::BrainFact;
use systemprompt_traits::RepositoryError;

use super::facts::FactRepository;
use crate::models::{FactCursor, FactFilter};

impl FactRepository {
    pub async fn list_reachable(
        &self,
        filter: &FactFilter,
        keys: &[String],
        after: Option<&FactCursor>,
    ) -> Result<Vec<BrainFact>, RepositoryError> {
        let statuses: Vec<String> = filter
            .statuses
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect();
        let mut conn = self.pools.read.acquire().await?;
        let ids = sqlx::query_scalar!(
            r#"SELECT f.id AS "id: BrainFactId" FROM brain_facts f
               WHERE EXISTS (SELECT 1 FROM v_brain_working_set w
                             WHERE w.fact_id = f.id AND w.scope_key = ANY($7::text[]))
                 AND ($1::text IS NULL OR f.kind = $1)
                 AND ($2::text IS NULL OR f.subject_kind = $2)
                 AND ($3::text IS NULL OR f.subject_id = $3)
                 AND (cardinality($4::text[]) = 0 OR f.status = ANY($4))
                 AND ($6::timestamptz IS NULL OR (
                     f.observed_at <= $6
                     AND (f.valid_from IS NULL OR f.valid_from <= $6)
                     AND (f.valid_to IS NULL OR f.valid_to > $6)))
                 AND ($8::timestamptz IS NULL OR (f.updated_at, f.id) < ($8, $9::text))
               ORDER BY f.updated_at DESC, f.id DESC LIMIT $5"#,
            filter.kind.as_ref().map(FactKindId::as_str),
            filter.subject.as_ref().map(|s| s.kind.as_str()),
            filter.subject.as_ref().map(|s| s.id.as_str()),
            &statuses,
            filter.limit,
            filter.as_of,
            keys,
            after.map(|c| c.updated_at),
            after.map(|c| c.id.as_str()),
        )
        .fetch_all(&mut *conn)
        .await?;
        Self::load(&mut *conn, &ids).await
    }
}
