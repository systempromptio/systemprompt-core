//! [`SearchTermsRepository`]: the document frequency of every lexeme in
//! `brain_facts.search_weighted`, refreshed by the consumer sweep and read
//! by the full-text tier's query planner (the recall store caches them).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use sqlx::{PgConnection, PgExecutor};
use systemprompt_traits::RepositoryError;

/// Lexeme document frequencies over the live facts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SearchTerms {
    pub total: u64,
    pub ndoc: HashMap<String, u64>,
}

impl SearchTerms {
    #[must_use]
    pub fn share(&self, word: &str) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        // Why: lint-ok: a ratio of counts; precision loss past 2^53 is irrelevant.
        self.ndoc_of(word) as f64 / self.total as f64
    }

    #[must_use]
    pub fn ndoc_of(&self, word: &str) -> u64 {
        self.ndoc.get(word).copied().unwrap_or(0)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total == 0
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SearchTermsRepository;

impl SearchTermsRepository {
    pub async fn refresh(conn: &mut PgConnection) -> Result<u64, RepositoryError> {
        sqlx::query!("DELETE FROM brain_search_terms")
            .execute(&mut *conn)
            .await?;
        let written = sqlx::query!(
            r"INSERT INTO brain_search_terms (word, ndoc)
              SELECT word, ndoc FROM ts_stat(
                  'SELECT search_weighted FROM brain_facts
                   WHERE status IN (''confirmed'', ''proposed'', ''stale'')'
              )"
        )
        .execute(&mut *conn)
        .await?
        .rows_affected();
        Ok(written)
    }

    pub async fn load(executor: impl PgExecutor<'_>) -> Result<SearchTerms, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT word, ndoc,
                      (SELECT count(*) FROM brain_facts
                       WHERE status IN ('confirmed', 'proposed', 'stale')) AS "total!"
               FROM brain_search_terms"#
        )
        .fetch_all(executor)
        .await?;
        let total = rows
            .first()
            .map_or(0, |row| u64::try_from(row.total).unwrap_or(0));
        let ndoc = rows
            .into_iter()
            .map(|row| (row.word, u64::try_from(row.ndoc).unwrap_or(0)))
            .collect();
        Ok(SearchTerms { total, ndoc })
    }
}
