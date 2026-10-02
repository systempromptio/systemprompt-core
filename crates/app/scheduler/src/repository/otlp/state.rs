//! The `otlp_export_state` rows: one cursor per signal, and the counters the
//! admin surface reads.
//!
//! A row is created with the watermark at "now" on the first export and is
//! never moved by a read. `advance` moves it only after the collector
//! acknowledged the batch; `record_failure` keeps it and stores the error;
//! `mark_caught_up` walks it to the edge of the settle window when a tail
//! found nothing, so `lag_seconds` (now minus watermark) reads as the
//! settle window while idle rather than growing without bound.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_models::profile::OtlpSignal;

use crate::error::SchedulerResult;

/// A `(timestamp, id)` cursor: every tail orders by both, so two rows with
/// the same timestamp cannot be skipped or shipped twice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Watermark {
    pub at: DateTime<Utc>,
    pub id: String,
}

impl Watermark {
    #[must_use]
    pub fn new(at: DateTime<Utc>, id: impl Into<String>) -> Self {
        Self { at, id: id.into() }
    }

    #[must_use]
    pub fn admits(&self, at: DateTime<Utc>, id: &str) -> bool {
        at > self.at || (at == self.at && id > self.id.as_str())
    }
}

/// One row of `otlp_export_state`, as the console shows it.
#[derive(Debug, Clone, Serialize)]
pub struct OtlpExportState {
    pub signal: String,
    pub watermark: DateTime<Utc>,
    pub watermark_id: String,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub batches_total: i64,
    pub failures_total: i64,
    pub rows_total: i64,
    pub lag_seconds: i64,
}

impl OtlpExportState {
    #[must_use]
    pub fn watermark(&self) -> Watermark {
        Watermark::new(self.watermark, self.watermark_id.clone())
    }
}

#[derive(Debug, Clone)]
pub struct OtlpExportStateRepository {
    pool: PgPool,
}

impl OtlpExportStateRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_states(&self) -> SchedulerResult<Vec<OtlpExportState>> {
        let rows = sqlx::query_as!(
            OtlpExportState,
            r#"
            SELECT signal, watermark, watermark_id, last_attempt_at, last_success_at,
                   last_error, last_error_at, batches_total, failures_total, rows_total,
                   EXTRACT(EPOCH FROM (NOW() - watermark))::BIGINT AS "lag_seconds!"
            FROM otlp_export_state
            ORDER BY signal
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_or_start(&self, signal: OtlpSignal) -> SchedulerResult<OtlpExportState> {
        let row = sqlx::query_as!(
            OtlpExportState,
            r#"
            INSERT INTO otlp_export_state (signal)
            VALUES ($1)
            ON CONFLICT (signal) DO UPDATE SET signal = EXCLUDED.signal
            RETURNING signal, watermark, watermark_id, last_attempt_at, last_success_at,
                      last_error, last_error_at, batches_total, failures_total, rows_total,
                      EXTRACT(EPOCH FROM (NOW() - watermark))::BIGINT AS "lag_seconds!"
            "#,
            signal.label()
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn mark_attempt(&self, signal: OtlpSignal) -> SchedulerResult<()> {
        sqlx::query!(
            "UPDATE otlp_export_state SET last_attempt_at = NOW(), updated_at = NOW() WHERE \
             signal = $1",
            signal.label()
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn advance(
        &self,
        signal: OtlpSignal,
        to: &Watermark,
        rows: i64,
    ) -> SchedulerResult<()> {
        sqlx::query!(
            r#"
            UPDATE otlp_export_state
            SET watermark = $2, watermark_id = $3, last_success_at = NOW(),
                last_error = NULL, last_error_at = NULL,
                batches_total = batches_total + 1, rows_total = rows_total + $4,
                updated_at = NOW()
            WHERE signal = $1
            "#,
            signal.label(),
            to.at,
            to.id,
            rows
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_failure(&self, signal: OtlpSignal, error: &str) -> SchedulerResult<()> {
        sqlx::query!(
            r#"
            UPDATE otlp_export_state
            SET last_error = $2, last_error_at = NOW(), failures_total = failures_total + 1,
                updated_at = NOW()
            WHERE signal = $1
            "#,
            signal.label(),
            error
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_caught_up(
        &self,
        signal: OtlpSignal,
        settle: std::time::Duration,
    ) -> SchedulerResult<()> {
        sqlx::query!(
            r#"
            UPDATE otlp_export_state
            SET watermark = GREATEST(watermark, NOW() - make_interval(secs => $2::DOUBLE PRECISION)),
                watermark_id = CASE
                    WHEN NOW() - make_interval(secs => $2::DOUBLE PRECISION) > watermark THEN ''
                    ELSE watermark_id
                END,
                last_success_at = NOW(), updated_at = NOW()
            WHERE signal = $1
            "#,
            signal.label(),
            settle.as_secs_f64()
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
