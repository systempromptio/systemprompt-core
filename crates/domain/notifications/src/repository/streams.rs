//! [`NotificationStreamRepository`]: declared streams, written only by the
//! notifications config plane.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgExecutor;
use systemprompt_identifiers::NotificationStreamId;
use systemprompt_models::notifications::{Attention, AudienceSubject, StreamAttention, StreamDecl};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{parse, parse_opt, to_i32, to_u32};

struct StreamRow {
    id: NotificationStreamId,
    name: String,
    description: Option<String>,
    audience: Vec<String>,
    publishers: Vec<String>,
    attention_default: String,
    attention_floor: Option<String>,
    attention_ceiling: Option<String>,
    retention_days: Option<i32>,
    coalesce_window_secs: Option<i32>,
}

impl TryFrom<StreamRow> for StreamDecl {
    type Error = RepositoryError;

    fn try_from(row: StreamRow) -> Result<Self, Self::Error> {
        let subjects = |field: &'static str, values: &[String]| {
            values
                .iter()
                .map(|v| parse::<AudienceSubject>(field, v))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            id: row.id,
            name: row.name,
            description: row.description,
            audience: subjects("notification_streams.audience", &row.audience)?,
            publishers: subjects("notification_streams.publishers", &row.publishers)?,
            attention: StreamAttention {
                default: parse(
                    "notification_streams.attention_default",
                    &row.attention_default,
                )?,
                floor: parse_opt(
                    "notification_streams.attention_floor",
                    row.attention_floor.as_deref(),
                )?,
                ceiling: parse_opt(
                    "notification_streams.attention_ceiling",
                    row.attention_ceiling.as_deref(),
                )?,
            },
            retention_days: to_u32("notification_streams.retention_days", row.retention_days)?,
            coalesce_window_secs: to_u32(
                "notification_streams.coalesce_window_secs",
                row.coalesce_window_secs,
            )?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NotificationStreamRepository {
    pools: Pools,
}

impl NotificationStreamRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn find(
        executor: impl PgExecutor<'_>,
        id: &NotificationStreamId,
    ) -> Result<Option<StreamDecl>, RepositoryError> {
        sqlx::query_as!(
            StreamRow,
            r#"SELECT id AS "id: NotificationStreamId", name, description, audience, publishers,
                      attention_default, attention_floor, attention_ceiling, retention_days,
                      coalesce_window_secs
               FROM notification_streams WHERE id = $1"#,
            id.as_str()
        )
        .fetch_optional(executor)
        .await?
        .map(StreamDecl::try_from)
        .transpose()
    }

    pub async fn find_pooled(
        &self,
        id: &NotificationStreamId,
    ) -> Result<Option<StreamDecl>, RepositoryError> {
        Self::find(&*self.pools.read, id).await
    }

    pub async fn list(executor: impl PgExecutor<'_>) -> Result<Vec<StreamDecl>, RepositoryError> {
        sqlx::query_as!(
            StreamRow,
            r#"SELECT id AS "id: NotificationStreamId", name, description, audience, publishers,
                      attention_default, attention_floor, attention_ceiling, retention_days,
                      coalesce_window_secs
               FROM notification_streams ORDER BY id"#
        )
        .fetch_all(executor)
        .await?
        .into_iter()
        .map(StreamDecl::try_from)
        .collect()
    }

    pub async fn list_pooled(&self) -> Result<Vec<StreamDecl>, RepositoryError> {
        Self::list(&*self.pools.read).await
    }

    pub async fn upsert(
        executor: impl PgExecutor<'_>,
        stream: &StreamDecl,
        source: &str,
    ) -> Result<(), RepositoryError> {
        let audience: Vec<String> = stream.audience.iter().map(ToString::to_string).collect();
        let publishers: Vec<String> = stream.publishers.iter().map(ToString::to_string).collect();
        sqlx::query!(
            "INSERT INTO notification_streams
                 (id, name, description, audience, publishers, attention_default,
                  attention_floor, attention_ceiling, retention_days, coalesce_window_secs, source)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (id) DO UPDATE SET
                 name = EXCLUDED.name, description = EXCLUDED.description,
                 audience = EXCLUDED.audience, publishers = EXCLUDED.publishers,
                 attention_default = EXCLUDED.attention_default,
                 attention_floor = EXCLUDED.attention_floor,
                 attention_ceiling = EXCLUDED.attention_ceiling,
                 retention_days = EXCLUDED.retention_days,
                 coalesce_window_secs = EXCLUDED.coalesce_window_secs,
                 source = EXCLUDED.source, updated_at = now()",
            stream.id.as_str(),
            stream.name,
            stream.description,
            &audience,
            &publishers,
            stream.attention.default.as_str(),
            stream.attention.floor.map(Attention::as_str),
            stream.attention.ceiling.map(Attention::as_str),
            to_i32("retention_days", stream.retention_days)?,
            to_i32("coalesce_window_secs", stream.coalesce_window_secs)?,
            source,
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn delete(
        executor: impl PgExecutor<'_>,
        id: &NotificationStreamId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM notification_streams WHERE id = $1",
            id.as_str()
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_retention(
        &self,
    ) -> Result<Vec<(NotificationStreamId, u32)>, RepositoryError> {
        sqlx::query!(
            r#"SELECT id AS "id: NotificationStreamId", retention_days AS "retention_days!"
               FROM notification_streams WHERE retention_days IS NOT NULL"#
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(|row| {
            let days = to_u32(
                "notification_streams.retention_days",
                Some(row.retention_days),
            )?
            .unwrap_or_default();
            Ok((row.id, days))
        })
        .collect()
    }
}
