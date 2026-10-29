//! The `licence_checkin_state` row: the last outcome of the report-only
//! licence check-in, as `admin licence status` shows it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::error::SchedulerResult;

/// The single `licence_checkin_state` row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LicenceCheckinState {
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_status: Option<i32>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub attempts_total: i64,
    pub failures_total: i64,
}

#[derive(Debug, Clone)]
pub struct LicenceCheckinRepository {
    pool: PgPool,
}

impl LicenceCheckinRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_state(&self) -> SchedulerResult<Option<LicenceCheckinState>> {
        let row = sqlx::query_as!(
            LicenceCheckinState,
            r#"
            SELECT last_attempt_at, last_success_at, last_status, last_error, last_error_at,
                   attempts_total, failures_total
            FROM licence_checkin_state
            WHERE id
            "#
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn mark_attempt(&self) -> SchedulerResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO licence_checkin_state (id, last_attempt_at, attempts_total)
            VALUES (TRUE, NOW(), 1)
            ON CONFLICT (id) DO UPDATE SET
                last_attempt_at = NOW(),
                attempts_total = licence_checkin_state.attempts_total + 1,
                updated_at = NOW()
            "#
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_success(&self, status: u16) -> SchedulerResult<()> {
        sqlx::query!(
            r#"
            UPDATE licence_checkin_state
            SET last_success_at = NOW(), last_status = $1, last_error = NULL,
                last_error_at = NULL, updated_at = NOW()
            WHERE id
            "#,
            i32::from(status)
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_failure(&self, status: Option<u16>, error: &str) -> SchedulerResult<()> {
        sqlx::query!(
            r#"
            UPDATE licence_checkin_state
            SET last_status = $1, last_error = $2, last_error_at = NOW(),
                failures_total = failures_total + 1, updated_at = NOW()
            WHERE id
            "#,
            status.map(i32::from),
            error
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
