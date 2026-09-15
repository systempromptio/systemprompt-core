//! Analytics reporting lifecycle on the shared durable event outbox.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rebuild;
mod status;
mod worker;

pub use rebuild::{initialize, rebuild};
pub use status::{ReportingStatus, status};
pub use worker::{process_pending, spawn};

use sqlx::PgConnection;
use systemprompt_analytics::AnalyticsError;

const PROJECTOR_LOCK: i64 = 0x5350_414e_414c_5954;

async fn lock(connection: &mut PgConnection) -> Result<(), AnalyticsError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PROJECTOR_LOCK)
        .execute(connection)
        .await?;
    Ok(())
}
