//! A person disputing, retracting or editing a fact is a correction of
//! every recall that injected it within the profile's correction window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use sqlx::PgExecutor;
use systemprompt_identifiers::{BrainFactId, UserId};

use crate::error::BrainResult;
use crate::repository::OutcomeRepository;

pub async fn on_correction(
    executor: impl PgExecutor<'_>,
    fact_id: &BrainFactId,
    actor: &UserId,
    window_days: u32,
) -> BrainResult<u64> {
    let now = Utc::now();
    let since = now - Duration::days(i64::from(window_days));
    let written = OutcomeRepository::record_corrections(executor, fact_id, since, now).await?;
    if written > 0 {
        tracing::info!(
            fact_id = %fact_id,
            actor = %actor,
            recalls = written,
            "Brain fact correction recorded against its recalls"
        );
    }
    Ok(written)
}
