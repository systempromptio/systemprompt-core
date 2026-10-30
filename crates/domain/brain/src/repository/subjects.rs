//! [`SubjectRepository`] and [`AliasRepository`]: the business subjects
//! facts are about and the names they go by, matched exactly or by
//! trigram similarity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_models::brain::SubjectKey;
use systemprompt_traits::RepositoryError;

use super::Pools;

/// A subject whose alias resembles the text looked up, with the trigram
/// similarity of the best alias.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasMatch {
    pub subject: SubjectKey,
    pub alias: String,
    pub similarity: f64,
}

#[derive(Debug, Clone)]
pub struct SubjectRepository {
    pools: Pools,
}

impl SubjectRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn ensure(
        conn: &mut PgConnection,
        subject: &SubjectKey,
        display_name: Option<&str>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_subjects (subject_kind, subject_id, display_name)
             VALUES ($1, $2, $3)
             ON CONFLICT (subject_kind, subject_id) DO UPDATE
             SET display_name = COALESCE(EXCLUDED.display_name, brain_subjects.display_name)",
            subject.kind,
            subject.id,
            display_name,
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn exists(&self, subject: &SubjectKey) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM brain_subjects
                              WHERE subject_kind = $1 AND subject_id = $2) AS "exists!""#,
            subject.kind,
            subject.id,
        )
        .fetch_one(&*self.pools.read)
        .await?)
    }
}

#[derive(Debug, Clone)]
pub struct AliasRepository {
    pools: Pools,
}

impl AliasRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    #[must_use]
    pub fn normalise(alias: &str) -> String {
        alias
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }

    pub async fn upsert(
        conn: &mut PgConnection,
        subject: &SubjectKey,
        alias: &str,
    ) -> Result<(), RepositoryError> {
        let id = uuid_text();
        sqlx::query!(
            "INSERT INTO brain_aliases (id, subject_kind, subject_id, alias, alias_norm)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (subject_kind, alias_norm) DO UPDATE
             SET subject_id = EXCLUDED.subject_id, alias = EXCLUDED.alias",
            id,
            subject.kind,
            subject.id,
            alias,
            Self::normalise(alias),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn similar(
        executor: impl PgExecutor<'_>,
        subject_kind: &str,
        text: &str,
        threshold: f32,
    ) -> Result<Vec<AliasMatch>, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT subject_kind, subject_id, alias,
                      similarity(alias_norm, $2)::float8 AS "similarity!"
               FROM brain_aliases
               WHERE subject_kind = $1 AND similarity(alias_norm, $2) >= $3::float4
               ORDER BY 4 DESC, subject_id LIMIT 20"#,
            subject_kind,
            Self::normalise(text),
            threshold,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| AliasMatch {
                subject: SubjectKey::new(row.subject_kind, row.subject_id),
                alias: row.alias,
                similarity: row.similarity,
            })
            .collect())
    }

    pub async fn similar_to(
        &self,
        subject_kind: &str,
        text: &str,
        threshold: f32,
    ) -> Result<Vec<AliasMatch>, RepositoryError> {
        Self::similar(&*self.pools.read, subject_kind, text, threshold).await
    }
}

fn uuid_text() -> String {
    uuid::Uuid::new_v4().to_string()
}
