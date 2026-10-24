//! `session_traffic_rollup_hourly` reads: the write-time counter behind the
//! traffic totals analytics reports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_models::analytics::TrafficClass;
use systemprompt_traits::{RepositoryError, TrafficClassCounts, TrafficClassTotals};

#[derive(Debug, Clone)]
pub struct SessionTrafficRollupRepository {
    pool: Arc<PgPool>,
}

impl SessionTrafficRollupRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }

    pub async fn rebuild(&self) -> Result<i64, RepositoryError> {
        let written =
            sqlx::query_scalar!(r#"SELECT session_traffic_rollup_rebuild() AS "written!""#)
                .fetch_one(&*self.pool)
                .await?;
        Ok(written)
    }
}

#[async_trait]
impl TrafficClassCounts for SessionTrafficRollupRepository {
    async fn session_totals(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TrafficClassTotals, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT traffic_class, COALESCE(SUM(sessions), 0)::bigint AS "sessions!"
            FROM session_traffic_rollup_hourly
            WHERE bucket >= date_trunc('hour', $1::timestamptz) AND bucket < $2
            GROUP BY traffic_class
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut totals = TrafficClassTotals::default();
        for row in rows {
            let class = TrafficClass::parse(&row.traffic_class).ok_or_else(|| {
                RepositoryError::decode(
                    "traffic_class",
                    std::io::Error::new(std::io::ErrorKind::InvalidData, row.traffic_class.clone()),
                )
            })?;
            match class {
                TrafficClass::Human => totals.human = row.sessions,
                TrafficClass::Ghost => totals.ghost = row.sessions,
                TrafficClass::Bot => totals.bot = row.sessions,
            }
        }
        Ok(totals)
    }
}
