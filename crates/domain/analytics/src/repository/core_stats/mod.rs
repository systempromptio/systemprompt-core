//! Core platform statistics aggregated from analytics tables.
//!
//! [`CoreStatsRepository`] backs the dashboard overview, activity trends,
//! categorical breakdowns, and leaderboards, with the read queries split
//! across the sibling submodules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod activity;
mod breakdowns;
mod leaderboards;
mod overview;

use crate::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug)]
pub struct CoreStatsRepository {
    pool: Arc<PgPool>,
}

impl CoreStatsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }
}
