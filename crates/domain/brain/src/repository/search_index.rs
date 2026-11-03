//! [`SearchIndexRepository`]: the weighted search vector on
//! `brain_facts.search_weighted`. Identity field values and the subject id
//! weigh `A`, every other field value `B`, and the statement with its
//! synonym expansions `C`. The field split comes from the kind declaration
//! in `brain_fact_kinds`; a fact whose kind is no longer declared indexes
//! its statement alone. The expansions are computed in Rust from the
//! `synonyms` config document and passed in, so one function produces them
//! for a single refresh and for a full rebuild.
//!
//! The generated `search` column stays for one release beside this one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::SynonymsDoc;
use systemprompt_traits::RepositoryError;

use super::SynonymsRepository;

const REBUILD_PAGE: i64 = 500;

#[derive(Debug, Clone, Copy, Default)]
pub struct SearchIndexRepository;

impl SearchIndexRepository {
    /// Recomputes one fact's vector; `expansion` is the synonym text to
    /// index with the statement.
    pub async fn refresh(
        conn: &mut PgConnection,
        id: &BrainFactId,
        expansion: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r"UPDATE brain_facts f SET search_weighted =
                  setweight(to_tsvector('simple',
                      f.subject_id || ' ' || COALESCE((
                          SELECT string_agg(f.fields ->> i, ' ')
                          FROM jsonb_array_elements_text(COALESCE(k.body -> 'identity', '[]'::jsonb)) i
                          WHERE f.fields ? i), '')), 'A')
                  || setweight(to_tsvector('simple', COALESCE((
                          SELECT string_agg(e.value, ' ')
                          FROM jsonb_each_text(f.fields) e
                          WHERE NOT COALESCE(k.body -> 'identity', '[]'::jsonb) ? e.key), '')), 'B')
                  || setweight(to_tsvector('simple', f.statement || ' ' || $2), 'C')
              FROM brain_facts f2
              LEFT JOIN brain_fact_kinds k ON k.id = f2.kind
              WHERE f2.id = f.id AND f.id = $1",
            id.as_str(),
            expansion,
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Recomputes one fact's vector with the stored synonyms document.
    pub async fn refresh_with_stored_synonyms(
        conn: &mut PgConnection,
        id: &BrainFactId,
    ) -> Result<(), RepositoryError> {
        let synonyms = SynonymsRepository::load(&mut *conn).await?;
        let statement = sqlx::query_scalar!(
            "SELECT statement FROM brain_facts WHERE id = $1",
            id.as_str()
        )
        .fetch_optional(&mut *conn)
        .await?;
        let Some(statement) = statement else {
            return Ok(());
        };
        Self::refresh(conn, id, &synonyms.expansion_text(&statement)).await
    }

    /// Recomputes every fact's vector with `synonyms`, in pages; answers
    /// the number of facts rewritten.
    pub async fn rebuild_all(
        conn: &mut PgConnection,
        synonyms: &SynonymsDoc,
    ) -> Result<u64, RepositoryError> {
        let mut rewritten = 0;
        let mut after = String::new();
        loop {
            let page = Self::page(&mut *conn, &after).await?;
            let Some(last) = page.last() else {
                break;
            };
            after.clone_from(&last.0);
            for (id, statement) in &page {
                let id = BrainFactId::try_new(id.clone())
                    .map_err(|e| RepositoryError::decode("brain_facts.id", e))?;
                Self::refresh(&mut *conn, &id, &synonyms.expansion_text(statement)).await?;
                rewritten += 1;
            }
        }
        Ok(rewritten)
    }

    async fn page(
        executor: impl PgExecutor<'_>,
        after: &str,
    ) -> Result<Vec<(String, String)>, RepositoryError> {
        let rows = sqlx::query!(
            "SELECT id, statement FROM brain_facts WHERE id > $1 ORDER BY id LIMIT $2",
            after,
            REBUILD_PAGE,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.id, row.statement))
            .collect())
    }

    /// The lexemes of one fact's weighted vector, as Postgres prints them,
    /// for tests and the console.
    pub async fn vector_text(
        executor: impl PgExecutor<'_>,
        id: &BrainFactId,
    ) -> Result<Option<String>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT search_weighted::text AS "vector!" FROM brain_facts WHERE id = $1"#,
            id.as_str()
        )
        .fetch_optional(executor)
        .await?)
    }
}
