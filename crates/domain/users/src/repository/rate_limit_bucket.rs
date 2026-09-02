//! Persistence for `user_rate_limit_buckets`: the replica-shared counters
//! behind the global per-user HTTP rate limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

use crate::error::Result;

#[derive(Clone, Debug)]
pub struct UserRateLimitBucketRepository {
    write_pool: Arc<PgPool>,
}

impl UserRateLimitBucketRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let write_pool = db.write_pool_arc()?;
        Ok(Self { write_pool })
    }

    pub async fn hit(
        &self,
        user_id: &UserId,
        scope: &str,
        window_start: DateTime<Utc>,
    ) -> Result<i64> {
        let row = sqlx::query!(
            r#"
            INSERT INTO user_rate_limit_buckets (user_id, scope, window_start, hits)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (user_id, scope, window_start)
            DO UPDATE SET hits = user_rate_limit_buckets.hits + 1
            RETURNING hits
            "#,
            user_id.as_str(),
            scope,
            window_start,
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(row.hits)
    }

    pub async fn prune(&self, before: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query!(
            r#"DELETE FROM user_rate_limit_buckets WHERE window_start < $1"#,
            before,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
