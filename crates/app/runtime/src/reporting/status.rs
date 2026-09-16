//! Reporting bootstrap state and durable queue lag.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_analytics::AnalyticsError;
use systemprompt_analytics::projection::{self, REPORTING_CONSUMER};
use systemprompt_database::DbPool;

use crate::RuntimeResult;

pub type ReportingStatus = projection::ProjectionStatus;

pub async fn status(db: &DbPool) -> RuntimeResult<ReportingStatus> {
    let pool = db.write_pool_arc()?;
    from_pool(&pool).await.map_err(Into::into)
}

pub(super) async fn from_pool(pool: &sqlx::PgPool) -> Result<ReportingStatus, AnalyticsError> {
    projection::status(pool, REPORTING_CONSUMER).await
}
