//! Durable aggregate snapshots, bounded range jobs and coordinated retention.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod daily;
mod expiry;
mod histogram;
mod jobs;
mod processing;
mod ranges;
mod reads;
mod retention;
mod retention_owners;
mod types;
pub use histogram::LatencyHistogram;
pub use types::*;

#[derive(Debug, Clone)]
/// `PostgreSQL` persistence for retained snapshots, jobs, and coordinated
/// compaction.
pub struct FeedbackSnapshotsRepository {
    pool: sqlx::PgPool,
    facts: crate::feedback::FeedbackFactsRepository,
}
impl FeedbackSnapshotsRepository {
    pub const fn new(pool: sqlx::PgPool, facts: crate::feedback::FeedbackFactsRepository) -> Self {
        Self { pool, facts }
    }
}
fn invalid(message: &str) -> crate::AnalyticsError {
    crate::AnalyticsError::invalid_argument(message)
}
